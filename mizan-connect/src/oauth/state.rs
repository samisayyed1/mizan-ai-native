//! OAuth `state` parameter HMAC signing + verification.
//!
//! Per ADR 0025 + working-agreement §16, the `state` param on the
//! OAuth authorization redirect is HMAC-signed by Mizan Connect so
//! the callback handler can reject forged callbacks before any token
//! exchange runs (CSRF defence).
//!
//! Layout of the signed token:
//!
//!   base64url( nonce(16) || sig(32) ) . base64url( payload_json )
//!
//! - `nonce(16)` — random 128-bit token-id; lets us de-dup replays
//!   if we ever bolt on a server-side cache (out of scope for PR-J1.b)
//! - `sig(32)`   — HMAC-SHA256 of (nonce || payload_json) under
//!   `OAUTH_STATE_SECRET` (env-loaded, see config.rs)
//! - `payload`   — JSON with the user id + provider slug + issued-at
//!   timestamp. Verifier checks issued-at is within 10 minutes (the
//!   authorize-then-callback window every provider documents).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use time::OffsetDateTime;
use uuid::Uuid;

use super::types::Provider;

type HmacSha256 = Hmac<Sha256>;

const NONCE_LEN: usize = 16;
const SIG_LEN: usize = 32;
const MAX_AGE_SECS: i64 = 600;

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("state token is malformed (missing separator)")]
    Malformed,
    #[error("state token signature does not verify — possible CSRF attempt")]
    InvalidSignature,
    #[error("state token payload is not valid JSON")]
    InvalidPayload,
    #[error("state token is older than {MAX_AGE_SECS} seconds (re-initiate the flow)")]
    Expired,
    #[error("state token nonce/signature length is wrong")]
    InvalidLength,
    #[error("OAUTH_STATE_SECRET must be ≥ 32 bytes; got {0}")]
    SecretTooShort(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatePayload {
    pub user_id: Uuid,
    pub provider: Provider,
    /// Unix-timestamp (seconds) when the state was issued.
    pub issued_at: i64,
}

/// Sign a state payload. Returns the URL-safe token to embed in the
/// `state=` query param of the authorization redirect.
pub fn sign(payload: &StatePayload, secret: &[u8]) -> Result<String, StateError> {
    if secret.len() < 32 {
        return Err(StateError::SecretTooShort(secret.len()));
    }
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);

    let payload_json = serde_json::to_string(payload).map_err(|_| StateError::InvalidPayload)?;

    let sig = compute_sig(&nonce, payload_json.as_bytes(), secret);

    let mut head = Vec::with_capacity(NONCE_LEN + SIG_LEN);
    head.extend_from_slice(&nonce);
    head.extend_from_slice(&sig);

    Ok(format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(&head),
        URL_SAFE_NO_PAD.encode(payload_json.as_bytes())
    ))
}

/// Verify a state token. On success returns the original payload.
pub fn verify(token: &str, secret: &[u8]) -> Result<StatePayload, StateError> {
    if secret.len() < 32 {
        return Err(StateError::SecretTooShort(secret.len()));
    }
    let (head_b64, body_b64) = token.split_once('.').ok_or(StateError::Malformed)?;
    let head = URL_SAFE_NO_PAD
        .decode(head_b64)
        .map_err(|_| StateError::Malformed)?;
    let body = URL_SAFE_NO_PAD
        .decode(body_b64)
        .map_err(|_| StateError::Malformed)?;

    if head.len() != NONCE_LEN + SIG_LEN {
        return Err(StateError::InvalidLength);
    }

    let (nonce, presented_sig) = head.split_at(NONCE_LEN);
    let expected_sig = compute_sig(nonce, &body, secret);

    // Constant-time compare to avoid timing leaks (subtle::ConstantTimeEq
    // is already in the workspace via the admin handler's bearer check).
    use subtle::ConstantTimeEq;
    if presented_sig.ct_eq(&expected_sig).into() {
        let payload: StatePayload =
            serde_json::from_slice(&body).map_err(|_| StateError::InvalidPayload)?;
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if now - payload.issued_at > MAX_AGE_SECS {
            return Err(StateError::Expired);
        }
        Ok(payload)
    } else {
        Err(StateError::InvalidSignature)
    }
}

fn compute_sig(nonce: &[u8], payload: &[u8], secret: &[u8]) -> [u8; SIG_LEN] {
    // HMAC-SHA256 accepts keys of any length up to the SHA-256 block
    // size (64 bytes) and silently rehashes longer keys. Our public
    // entry points clamp the secret to ≥ 32 bytes, so this never
    // errors in practice — but we explicitly handle the impossible
    // case rather than `.expect()` to satisfy clippy::expect_used.
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return [0u8; SIG_LEN],
    };
    mac.update(nonce);
    mac.update(payload);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; SIG_LEN];
    out.copy_from_slice(&result);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixture_payload() -> StatePayload {
        StatePayload {
            user_id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            provider: Provider::GoogleDrive,
            issued_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }

    #[test]
    fn sign_then_verify_round_trips() {
        let secret = [0x42u8; 32];
        let payload = fixture_payload();
        let token = sign(&payload, &secret).unwrap();
        let back = verify(&token, &secret).unwrap();
        assert_eq!(back.user_id, payload.user_id);
        assert_eq!(back.provider, payload.provider);
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let secret = [0x42u8; 32];
        let payload = fixture_payload();
        let token = sign(&payload, &secret).unwrap();
        // Swap one character in the body (payload JSON).
        let mut t = token.into_bytes();
        let last = t.len() - 1;
        t[last] = t[last].wrapping_add(1);
        let err = verify(&String::from_utf8(t).unwrap(), &secret).unwrap_err();
        assert!(
            matches!(
                err,
                StateError::InvalidSignature | StateError::Malformed | StateError::InvalidPayload
            ),
            "{err:?}",
        );
    }

    #[test]
    fn verify_rejects_wrong_secret() {
        let signing_secret = [0x42u8; 32];
        let attacker_secret = [0xff_u8; 32];
        let token = sign(&fixture_payload(), &signing_secret).unwrap();
        assert!(matches!(
            verify(&token, &attacker_secret).unwrap_err(),
            StateError::InvalidSignature
        ));
    }

    #[test]
    fn verify_rejects_expired_state() {
        let secret = [0x42u8; 32];
        let stale = StatePayload {
            user_id: Uuid::nil(),
            provider: Provider::Notion,
            issued_at: OffsetDateTime::now_utc().unix_timestamp() - MAX_AGE_SECS - 5,
        };
        let token = sign(&stale, &secret).unwrap();
        assert!(matches!(
            verify(&token, &secret).unwrap_err(),
            StateError::Expired
        ));
    }

    #[test]
    fn sign_rejects_short_secret() {
        assert!(matches!(
            sign(&fixture_payload(), &[0x01_u8; 16]).unwrap_err(),
            StateError::SecretTooShort(16)
        ));
    }

    #[test]
    fn verify_rejects_malformed_token() {
        let secret = [0x42u8; 32];
        assert!(matches!(
            verify("nope-no-dot-here", &secret).unwrap_err(),
            StateError::Malformed
        ));
    }
}
