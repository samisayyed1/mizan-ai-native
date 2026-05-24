//! AES-256-GCM encryption for provider access tokens at rest.
//!
//! Mizan Connect stores long-lived Plaid access tokens only on the backend,
//! encrypted before they touch Postgres. The frontend receives Link tokens and
//! public tokens only; it never sees Plaid access tokens.

use std::sync::Arc;

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use secrecy::{ExposeSecret, SecretString};
use zeroize::Zeroize;

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum SecretCryptoError {
    #[error("encryption key must be exactly {KEY_LEN} bytes, got {0}")]
    InvalidKeyLength(usize),
    #[error("encryption failed")]
    EncryptFailed,
    #[error("decryption failed (truncated, wrong key, or tampered ciphertext)")]
    DecryptFailed,
    #[error("encryption self-test failed: round-trip mismatch")]
    SelfTestFailed,
    #[error("encrypted blob too short: need at least {} bytes, got {0}", NONCE_LEN + 16)]
    BlobTooShort(usize),
    #[error("plaintext is not valid UTF-8")]
    PlaintextNotUtf8,
}

#[derive(Clone)]
pub struct SecretCipher {
    inner: Arc<[u8; KEY_LEN]>,
}

impl std::fmt::Debug for SecretCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretCipher")
            .field("algorithm", &"AES-256-GCM")
            .finish()
    }
}

impl SecretCipher {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SecretCryptoError> {
        if bytes.len() != KEY_LEN {
            return Err(SecretCryptoError::InvalidKeyLength(bytes.len()));
        }
        let mut key = [0u8; KEY_LEN];
        key.copy_from_slice(bytes);
        Ok(Self {
            inner: Arc::new(key),
        })
    }

    fn cipher(&self) -> Aes256Gcm {
        let key = Key::<Aes256Gcm>::from_slice(self.inner.as_ref());
        Aes256Gcm::new(key)
    }

    pub fn encrypt(&self, plaintext: &SecretString) -> Result<Vec<u8>, SecretCryptoError> {
        let cipher = self.cipher();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.expose_secret().as_bytes())
            .map_err(|_| SecretCryptoError::EncryptFailed)?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn decrypt(&self, blob: &[u8]) -> Result<SecretString, SecretCryptoError> {
        if blob.len() < NONCE_LEN + 16 {
            return Err(SecretCryptoError::BlobTooShort(blob.len()));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = self.cipher();
        let mut plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| SecretCryptoError::DecryptFailed)?;
        let secret = String::from_utf8(plaintext.clone()).map_err(|_| {
            plaintext.zeroize();
            SecretCryptoError::PlaintextNotUtf8
        })?;
        plaintext.zeroize();
        Ok(SecretString::from(secret))
    }

    pub fn self_test(&self) -> Result<(), SecretCryptoError> {
        let plaintext = SecretString::from("mizan-plaid-secret-self-test".to_string());
        let blob = self.encrypt(&plaintext)?;
        let decrypted = self.decrypt(&blob)?;
        if decrypted.expose_secret() != plaintext.expose_secret() {
            return Err(SecretCryptoError::SelfTestFailed);
        }
        if blob == self.encrypt(&plaintext)? {
            return Err(SecretCryptoError::SelfTestFailed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn fixed_cipher() -> SecretCipher {
        SecretCipher::from_bytes(&[9u8; KEY_LEN]).expect("32 byte key")
    }

    #[test]
    fn round_trip_secret() {
        let cipher = fixed_cipher();
        let secret = SecretString::from("access-sandbox-secret".to_string());
        let blob = cipher.encrypt(&secret).expect("encrypt");
        let out = cipher.decrypt(&blob).expect("decrypt");
        assert_eq!(out.expose_secret(), secret.expose_secret());
    }

    #[test]
    fn tampered_blob_fails() {
        let cipher = fixed_cipher();
        let secret = SecretString::from("access-sandbox-secret".to_string());
        let mut blob = cipher.encrypt(&secret).expect("encrypt");
        let last = blob.len() - 1;
        blob[last] ^= 1;
        assert!(matches!(
            cipher.decrypt(&blob).unwrap_err(),
            SecretCryptoError::DecryptFailed
        ));
    }

    #[test]
    fn rejects_bad_key_length() {
        assert!(matches!(
            SecretCipher::from_bytes(&[1u8; 16]).unwrap_err(),
            SecretCryptoError::InvalidKeyLength(16)
        ));
    }
}
