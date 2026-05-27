//! Postgres-backed storage for SnapTrade per-user secrets.
//!
//! Uses the existing `broker_connections` table from migration 0001/0002.
//! Schema (columns we touch):
//! ```sql
//! user_id                          UUID NOT NULL
//! snaptrade_user_id                TEXT NOT NULL
//! snaptrade_user_secret_encrypted  BYTEA NOT NULL  -- AES-256-GCM(secret)
//! snaptrade_authorization_id       TEXT
//! is_active                        BOOLEAN NOT NULL
//! connection_type                  TEXT NOT NULL DEFAULT 'snaptrade'
//! ```
//! The partial UNIQUE index `uq_broker_conn_user_snaptrade` already
//! enforces "one active SnapTrade connection per user", so we use
//! `INSERT … ON CONFLICT … DO UPDATE` to keep registration idempotent.
//!
//! `snaptrade_user_secret_encrypted` is `nonce || ciphertext` per
//! AES-256-GCM. Key = `MIZAN_SNAPTRADE_TOKEN_ENCRYPTION_KEY`, 32 bytes
//! base64-decoded, distinct from Plaid's so a compromise of one
//! provider's key can't unlock the other.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key};

use crate::error::{AppError, ErrorCode};
use crate::state::AppState;

use super::client::SnapTradeClient;

const NONCE_LEN: usize = 12;

pub(super) fn encrypt(key: &[u8], plaintext: &str) -> Result<Vec<u8>, AppError> {
    if key.len() != 32 {
        return Err(AppError::new(
            ErrorCode::Internal,
            format!(
                "snaptrade token key must be 32 bytes (got {})",
                key.len()
            ),
        ));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let mut ct = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("encrypt: {e}")))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(nonce.as_slice());
    out.append(&mut ct);
    Ok(out)
}

pub(super) fn decrypt(key: &[u8], blob: &[u8]) -> Result<String, AppError> {
    use aes_gcm::Nonce;
    if blob.len() < NONCE_LEN {
        return Err(AppError::new(
            ErrorCode::Internal,
            "snaptrade ciphertext too short".to_string(),
        ));
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let (nonce_bytes, ct) = blob.split_at(NONCE_LEN);
    let nonce = Nonce::from_slice(nonce_bytes);
    let pt = cipher
        .decrypt(nonce, ct)
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("decrypt: {e}")))?;
    String::from_utf8(pt)
        .map_err(|e| AppError::new(ErrorCode::Internal, format!("decrypt utf8: {e}")))
}

/// Load the SnapTrade `(userId, userSecret)` for a Mizan user from
/// `broker_connections`. Returns `None` when this Mizan user has not
/// yet completed SnapTrade registration (no active row).
pub async fn load_user_credentials(
    state: &AppState,
    mizan_user_id: &uuid::Uuid,
    key: &[u8],
) -> Result<Option<(String, String)>, AppError> {
    let row: Option<(String, Vec<u8>)> = sqlx::query_as(
        r#"
        SELECT snaptrade_user_id, snaptrade_user_secret_encrypted
          FROM broker_connections
         WHERE user_id = $1
           AND connection_type = 'snaptrade'
           AND is_active = TRUE
         ORDER BY updated_at DESC
         LIMIT 1
        "#,
    )
    .bind(mizan_user_id)
    .fetch_optional(state.db())
    .await
    .map_err(|e| AppError::new(ErrorCode::Internal, format!("db: {e}")))?;

    match row {
        Some((uid, blob)) => {
            let secret = decrypt(key, &blob)?;
            Ok(Some((uid, secret)))
        }
        None => Ok(None),
    }
}

/// Idempotently register the Mizan user with SnapTrade.
///
/// Step 1: try to load existing credentials. On hit, return them
/// (no upstream API call).
/// Step 2: call SnapTrade `/snapTrade/registerUser`. The Mizan UUID
/// becomes the SnapTrade `userId` so the mapping is stable.
/// Step 3: persist the encrypted `userSecret` to `broker_connections`.
/// The partial UNIQUE index handles "one active per user" — we
/// upsert by `user_id` filtered to the active SnapTrade row.
pub async fn ensure_user_registered(
    state: &AppState,
    client: &SnapTradeClient,
    mizan_user_id: &uuid::Uuid,
    key: &[u8],
) -> Result<(String, String), AppError> {
    if let Some(creds) = load_user_credentials(state, mizan_user_id, key).await? {
        // Touch updated_at so we can age out abandoned registrations later.
        let _ = sqlx::query(
            "UPDATE broker_connections
                SET updated_at = NOW()
              WHERE user_id = $1 AND connection_type = 'snaptrade' AND is_active = TRUE",
        )
        .bind(mizan_user_id)
        .execute(state.db())
        .await;
        return Ok(creds);
    }

    let mizan_user_str = mizan_user_id.to_string();
    let reg = client.register_user(&mizan_user_str).await?;
    let encrypted = encrypt(key, &reg.user_secret)?;

    // The partial UNIQUE prevents two active rows; if a previously-
    // disabled row exists we insert a fresh active one alongside it.
    sqlx::query(
        r#"
        INSERT INTO broker_connections
            (user_id, snaptrade_user_id, snaptrade_user_secret_encrypted,
             connection_type, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, 'snaptrade', TRUE, NOW(), NOW())
        ON CONFLICT (user_id) WHERE (connection_type = 'snaptrade' AND is_active = TRUE)
        DO UPDATE
          SET snaptrade_user_id = EXCLUDED.snaptrade_user_id,
              snaptrade_user_secret_encrypted = EXCLUDED.snaptrade_user_secret_encrypted,
              updated_at = NOW()
        "#,
    )
    .bind(mizan_user_id)
    .bind(&reg.user_id)
    .bind(&encrypted)
    .execute(state.db())
    .await
    .map_err(|e| AppError::new(ErrorCode::Internal, format!("db: {e}")))?;

    Ok((reg.user_id, reg.user_secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_round_trips() {
        let key = vec![7u8; 32];
        let pt = "hunter2-very-secret";
        let blob = encrypt(&key, pt).unwrap();
        let blob2 = encrypt(&key, pt).unwrap();
        assert_ne!(blob, blob2);
        assert_eq!(decrypt(&key, &blob).unwrap(), pt);
        assert_eq!(decrypt(&key, &blob2).unwrap(), pt);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key = vec![1u8; 32];
        let blob = encrypt(&key, "x").unwrap();
        let wrong = vec![2u8; 32];
        assert!(decrypt(&wrong, &blob).is_err());
    }

    #[test]
    fn encrypt_rejects_wrong_key_length() {
        assert!(encrypt(&vec![0u8; 31], "x").is_err());
    }
}
