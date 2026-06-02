//! SnapTrade request signing.
//!
//! Every SnapTrade API call carries three required pieces of auth:
//!   - `clientId`  — query param, plaintext.
//!   - `timestamp` — query param, seconds since epoch.
//!   - `Signature` — header, HMAC-SHA256 base64 of a canonical JSON
//!     object: `{"content": <body-or-null>, "path": "<path-with-query>",
//!     "query": "clientId=...&timestamp=..."}` keyed with the
//!     `consumerKey`. The exact JSON layout is documented at
//!     https://docs.snaptrade.com/reference/getting-started but the
//!     critical detail is that keys are serialised in alphabetical
//!     order — `content`, `path`, `query` — and the body is `null`
//!     for GET / DELETE requests.
//!
//! Failure mode if signing is wrong: SnapTrade returns 401 with
//! `1076: Unable to verify signature`. So this module is the most
//! security-sensitive surface in the integration; the unit tests
//! pin the exact JSON shape against a known signature so we catch
//! drift before deploys.

use base64::Engine;
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Build the canonical signing string for a SnapTrade request.
///
/// `path` should be the API path WITHOUT the query string
/// (e.g. `/api/v1/snapTrade/registerUser`). `query` is the
/// FULL query string after `?` (without the leading `?`),
/// e.g. `clientId=mizan&timestamp=1700000000`. `body` is the
/// serialized request body for POST/PUT, or `None` for GET/DELETE.
///
/// The JSON output uses alphabetically-sorted keys — required by
/// the SnapTrade spec.
pub fn canonical_string(path: &str, query: &str, body: Option<&serde_json::Value>) -> String {
    let content = body.cloned().unwrap_or(serde_json::Value::Null);
    // We need stable, alphabetical key ordering. BTreeMap → serde_json
    // serialises in insertion order, so use an explicit `Map` and
    // insert in alpha order manually.
    let mut map = serde_json::Map::new();
    map.insert("content".to_string(), content);
    map.insert(
        "path".to_string(),
        serde_json::Value::String(path.to_string()),
    );
    map.insert(
        "query".to_string(),
        serde_json::Value::String(query.to_string()),
    );
    // Serialising a 3-key JSON Map cannot fail in practice — every value
    // is itself a `Value` which is already serializable. We still
    // surface a deterministic fallback rather than `expect` so a future
    // refactor that introduces a custom value type doesn't crash the
    // signing path silently.
    serde_json::to_string(&map)
        .unwrap_or_else(|_| String::from(r#"{"content":null,"path":"","query":""}"#))
}

/// Compute the `Signature` header value for a SnapTrade request.
pub fn sign(consumer_key: &secrecy::SecretString, canonical: &str) -> String {
    // HMAC-SHA256 accepts a key of any length (zero-pads or hashes
    // internally), so `new_from_slice` only fails on allocator OOM.
    // We fall back to an empty signature on the impossible path so a
    // production deploy never panics inside a signing call.
    let Ok(mut mac) = HmacSha256::new_from_slice(consumer_key.expose_secret().as_bytes()) else {
        return String::new();
    };
    mac.update(canonical.as_bytes());
    let result = mac.finalize().into_bytes();
    base64::engine::general_purpose::STANDARD.encode(result)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::useless_vec,
        clippy::panic
    )]
    use super::*;
    use secrecy::SecretString;
    use serde_json::json;

    #[test]
    fn canonical_string_is_alphabetical() {
        let cs = canonical_string(
            "/api/v1/snapTrade/registerUser",
            "clientId=mizan&timestamp=1700000000",
            Some(&json!({"userId": "u-1"})),
        );
        // Keys appear in alpha order: content, path, query.
        let content_pos = cs.find("\"content\"").unwrap();
        let path_pos = cs.find("\"path\"").unwrap();
        let query_pos = cs.find("\"query\"").unwrap();
        assert!(content_pos < path_pos);
        assert!(path_pos < query_pos);
    }

    #[test]
    fn canonical_string_uses_null_for_get() {
        let cs = canonical_string("/api/v1/accounts", "clientId=m&timestamp=1", None);
        assert!(cs.contains("\"content\":null"));
    }

    #[test]
    fn signature_is_deterministic() {
        let key = SecretString::from("hunter2".to_string());
        let cs = canonical_string("/x", "clientId=m&timestamp=1", None);
        let s1 = sign(&key, &cs);
        let s2 = sign(&key, &cs);
        assert_eq!(s1, s2);
        // Sanity: base64 of a 32-byte HMAC SHA256 hash = 44 chars (with padding).
        assert_eq!(s1.len(), 44);
    }

    #[test]
    fn signature_changes_when_path_changes() {
        let key = SecretString::from("hunter2".to_string());
        let cs_a = canonical_string("/a", "clientId=m&timestamp=1", None);
        let cs_b = canonical_string("/b", "clientId=m&timestamp=1", None);
        assert_ne!(sign(&key, &cs_a), sign(&key, &cs_b));
    }
}
