//! Plaid webhook signature verification (JWT ES256).
//!
//! Plaid sends each webhook with a `Plaid-Verification` header containing a
//! JWT signed by Plaid with ES256. The JWT body carries `iat` and a
//! `request_body_sha256` claim. We:
//!
//! 1. Parse the unverified JWT header to obtain the `kid`.
//! 2. Look up the corresponding public key from a small in-process cache,
//!    falling back to `/webhook_verification_key/get` if not cached.
//! 3. Verify the JWT signature with ES256 and the body hash claim against
//!    SHA-256 of the raw request body.
//! 4. Reject if the JWT is older than 5 minutes (replay window).
//!
//! Reference: https://plaid.com/docs/api/webhooks/webhook-verification/

use std::collections::HashMap;
use std::sync::Arc;

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use parking_lot::RwLock;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use super::client::PlaidClient;

/// Replay window: a Plaid JWT older than this is rejected.
const MAX_AGE_SECONDS: i64 = 300;

#[derive(Debug, thiserror::Error)]
pub enum WebhookVerifyError {
    #[error("missing Plaid-Verification header")]
    MissingHeader,
    #[error("malformed Plaid-Verification header")]
    MalformedHeader,
    #[error("plaid key id missing from token header")]
    MissingKid,
    #[error("plaid verification key unavailable")]
    KeyFetch,
    #[error("signature verification failed")]
    BadSignature,
    #[error("request body hash mismatch")]
    BodyHashMismatch,
    #[error("verification token expired")]
    Expired,
}

#[derive(Debug, Deserialize)]
pub struct WebhookKey {
    pub kid: String,
    #[serde(default)]
    pub expired_at: Option<i64>,
    pub alg: String,
    pub crv: String,
    pub kty: String,
    pub x: String,
    pub y: String,
}

#[derive(Debug, Deserialize)]
pub struct WebhookKeyResponse {
    pub key: WebhookKey,
}

/// Thread-safe cache of Plaid webhook verification keys keyed by `kid`.
#[derive(Debug, Default, Clone)]
pub struct WebhookKeyCache {
    inner: Arc<RwLock<HashMap<String, WebhookKey>>>,
}

impl WebhookKeyCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, kid: &str) -> Option<WebhookKey> {
        self.inner.read().get(kid).cloned()
    }

    pub fn put(&self, key: WebhookKey) {
        self.inner.write().insert(key.kid.clone(), key);
    }

    pub fn invalidate(&self, kid: &str) {
        self.inner.write().remove(kid);
    }
}

// `WebhookKey` is `Clone`-able because it is small JSON-derived data. Avoid
// deriving Clone on the struct above to keep the surface explicit.
impl Clone for WebhookKey {
    fn clone(&self) -> Self {
        Self {
            kid: self.kid.clone(),
            expired_at: self.expired_at,
            alg: self.alg.clone(),
            crv: self.crv.clone(),
            kty: self.kty.clone(),
            x: self.x.clone(),
            y: self.y.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PlaidJwtClaims {
    iat: i64,
    request_body_sha256: String,
}

/// Verify a Plaid webhook envelope.
///
/// `header_value` is the raw `Plaid-Verification` header. `body` is the raw
/// request bytes — must not be re-serialized, because Plaid hashes the
/// exact bytes sent.
pub async fn verify(
    client: &PlaidClient,
    cache: &WebhookKeyCache,
    header_value: &str,
    body: &[u8],
) -> Result<(), WebhookVerifyError> {
    if header_value.is_empty() {
        return Err(WebhookVerifyError::MissingHeader);
    }

    let unverified =
        decode_header(header_value).map_err(|_| WebhookVerifyError::MalformedHeader)?;
    let kid = unverified.kid.ok_or(WebhookVerifyError::MissingKid)?;

    let key = match cache.get(&kid) {
        Some(k) => k,
        None => {
            let response = client
                .webhook_verification_key_get(&kid)
                .await
                .map_err(|err| {
                    tracing::warn!(error = %err, plaid.kid = %kid, "plaid webhook key fetch failed");
                    WebhookVerifyError::KeyFetch
                })?;
            cache.put(response.key.clone());
            response.key
        }
    };

    if key.alg != "ES256" || key.kty != "EC" || key.crv != "P-256" {
        tracing::warn!(plaid.kid = %kid, alg = %key.alg, kty = %key.kty, crv = %key.crv, "unsupported plaid webhook key");
        return Err(WebhookVerifyError::BadSignature);
    }

    let decoding_key = DecodingKey::from_ec_components(&key.x, &key.y)
        .map_err(|_| WebhookVerifyError::BadSignature)?;

    let mut validation = Validation::new(Algorithm::ES256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    validation.validate_nbf = false;
    validation.validate_aud = false;

    let claims = decode::<PlaidJwtClaims>(header_value, &decoding_key, &validation)
        .map_err(|err| {
            tracing::warn!(error = %err, plaid.kid = %kid, "plaid jwt signature verification failed");
            WebhookVerifyError::BadSignature
        })?
        .claims;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    if now.saturating_sub(claims.iat) > MAX_AGE_SECONDS {
        return Err(WebhookVerifyError::Expired);
    }

    let mut hasher = Sha256::new();
    hasher.update(body);
    let computed_hex = hex::encode(hasher.finalize());

    if !constant_time_eq(claims.request_body_sha256.as_bytes(), computed_hex.as_bytes()) {
        return Err(WebhookVerifyError::BodyHashMismatch);
    }

    Ok(())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn cache_round_trip() {
        let cache = WebhookKeyCache::new();
        let key = WebhookKey {
            kid: "test-kid".into(),
            expired_at: None,
            alg: "ES256".into(),
            crv: "P-256".into(),
            kty: "EC".into(),
            x: "xx".into(),
            y: "yy".into(),
        };
        cache.put(key.clone());
        let fetched = cache.get("test-kid").expect("cached key should be present");
        assert_eq!(fetched.kid, "test-kid");
        cache.invalidate("test-kid");
        assert!(cache.get("test-kid").is_none());
    }

    #[test]
    fn constant_time_eq_correct() {
        assert!(constant_time_eq(b"abcdef", b"abcdef"));
        assert!(!constant_time_eq(b"abcdef", b"abcdee"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }
}
