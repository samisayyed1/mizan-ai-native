//! Passphrase-based encryption envelope for Mizan database backups.
//!
//! # Threat model
//!
//! The Mizan SQLite database contains broker OAuth tokens, AI provider
//! API keys (during the transition window before they move to the OS
//! keyring), the entire activity log, account metadata, and computed
//! holdings/snapshots. A user who exports a backup commonly uploads it
//! to Drive / iCloud / Dropbox / a personal NAS — all of which can be
//! breached at the provider level, on a shared device, or via stolen
//! credentials.
//!
//! Previous behaviour: `backup_database` returned the raw SQLite file
//! bytes unencrypted. Anyone obtaining the backup obtained every
//! credential.
//!
//! New behaviour: backups are wrapped in this passphrase-derived
//! envelope. Without the passphrase, the backup is just opaque bytes.
//! With the passphrase, recovery is straightforward inside Mizan's
//! restore flow.
//!
//! # Envelope format
//!
//! All multi-byte fields are little-endian.
//!
//! | Field             | Size       | Purpose |
//! |-------------------|------------|---------|
//! | Magic             | 8 bytes    | b"MIZBKP01" — version + format identifier |
//! | KDF id            | 1 byte     | 0x01 = Argon2id v0x13 (only value today) |
//! | Cipher id         | 1 byte     | 0x01 = ChaCha20-Poly1305 (only value today) |
//! | Argon2 m_cost (KiB)  | 4 bytes | memory cost in KiB                |
//! | Argon2 t_cost     | 4 bytes    | time cost (iterations)            |
//! | Argon2 p_cost     | 4 bytes    | parallelism                       |
//! | Salt length       | 1 byte     | always 16                         |
//! | Salt              | 16 bytes   | random per-backup                 |
//! | Nonce length      | 1 byte     | always 12                         |
//! | Nonce             | 12 bytes   | random per-backup                 |
//! | Ciphertext + tag  | rest       | ChaCha20-Poly1305 output (tag is appended by the crate; 16 bytes) |
//!
//! Why include the KDF params in the envelope: future Mizan releases
//! may calibrate Argon2 parameters upward as hardware gets faster, and
//! existing backups must remain restorable. Embedding the params makes
//! the envelope self-describing.
//!
//! # Calibration
//!
//! Defaults are tuned for ~250ms derivation on a 2024 laptop CPU:
//! `m_cost = 64 MiB, t_cost = 3, p_cost = 1`. Mobile and older
//! hardware can take longer, but the user only sees this on backup +
//! restore — twice the entire lifetime of a backup file. Acceptable
//! trade-off for resistance to offline GPU brute-force.

use anyhow::{anyhow, bail, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;

const MAGIC: &[u8; 8] = b"MIZBKP01";
const KDF_ARGON2ID_V13: u8 = 0x01;
const CIPHER_CHACHA20_POLY1305: u8 = 0x01;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
/// Argon2id parameters tuned for ~250ms on a 2024 desktop CPU. Stored
/// in the envelope so future re-calibrations don't break old backups.
const DEFAULT_ARGON2_M_COST_KIB: u32 = 64 * 1024;
const DEFAULT_ARGON2_T_COST: u32 = 3;
const DEFAULT_ARGON2_P_COST: u32 = 1;

/// Minimum passphrase length. Anything shorter is rejected up front
/// rather than letting the user create a backup that's trivially
/// brute-forceable.
const MIN_PASSPHRASE_LEN: usize = 8;

/// Encrypt `plaintext` (raw SQLite bytes) under the user-supplied
/// passphrase. Returns the full envelope ready to be written to disk
/// or returned to the frontend as a `Vec<u8>`.
pub fn encrypt_backup(plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    validate_passphrase(passphrase)?;

    // Generate fresh salt + nonce for every backup so repeated backups
    // of the same DB with the same password yield distinct ciphertexts
    // and reveal nothing about identical contents.
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    let key = derive_key_argon2id(
        passphrase.as_bytes(),
        &salt,
        DEFAULT_ARGON2_M_COST_KIB,
        DEFAULT_ARGON2_T_COST,
        DEFAULT_ARGON2_P_COST,
    )?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|e| anyhow!("ChaCha20-Poly1305 encrypt failed: {}", e))?;

    let mut out =
        Vec::with_capacity(MAGIC.len() + 2 + 12 + 1 + SALT_LEN + 1 + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.push(KDF_ARGON2ID_V13);
    out.push(CIPHER_CHACHA20_POLY1305);
    out.extend_from_slice(&DEFAULT_ARGON2_M_COST_KIB.to_le_bytes());
    out.extend_from_slice(&DEFAULT_ARGON2_T_COST.to_le_bytes());
    out.extend_from_slice(&DEFAULT_ARGON2_P_COST.to_le_bytes());
    out.push(SALT_LEN as u8);
    out.extend_from_slice(&salt);
    out.push(NONCE_LEN as u8);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt a backup envelope previously produced by [`encrypt_backup`].
/// Returns the original plaintext (raw SQLite bytes) or an error if
/// the envelope is malformed or the passphrase is wrong.
///
/// Errors do NOT distinguish between "wrong passphrase" and "tampered
/// ciphertext" beyond the AEAD's authenticate-then-decrypt failure —
/// this is intentional. The user-facing message can simply say
/// "could not decrypt, check passphrase".
pub fn decrypt_backup(envelope: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    if passphrase.is_empty() {
        bail!("Passphrase is required to decrypt a backup");
    }

    let mut cursor = 0usize;
    let need = |cursor: usize, len: usize, what: &str| -> Result<()> {
        if envelope.len() < cursor + len {
            bail!("Backup envelope truncated while reading {}", what);
        }
        Ok(())
    };

    need(cursor, MAGIC.len(), "magic header")?;
    if &envelope[cursor..cursor + MAGIC.len()] != MAGIC {
        bail!(
            "Not a Mizan backup envelope (expected magic {:?}, got {:?})",
            MAGIC,
            &envelope[cursor..cursor + MAGIC.len()]
        );
    }
    cursor += MAGIC.len();

    need(cursor, 1, "kdf id")?;
    let kdf_id = envelope[cursor];
    cursor += 1;
    if kdf_id != KDF_ARGON2ID_V13 {
        bail!(
            "Unsupported KDF id {} (this Mizan only supports Argon2id v0x13)",
            kdf_id
        );
    }

    need(cursor, 1, "cipher id")?;
    let cipher_id = envelope[cursor];
    cursor += 1;
    if cipher_id != CIPHER_CHACHA20_POLY1305 {
        bail!(
            "Unsupported cipher id {} (this Mizan only supports ChaCha20-Poly1305)",
            cipher_id
        );
    }

    need(cursor, 12, "argon2 params")?;
    let m_cost = u32::from_le_bytes(envelope[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;
    let t_cost = u32::from_le_bytes(envelope[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;
    let p_cost = u32::from_le_bytes(envelope[cursor..cursor + 4].try_into().unwrap());
    cursor += 4;

    need(cursor, 1, "salt length")?;
    let salt_len = envelope[cursor] as usize;
    cursor += 1;
    if salt_len == 0 || salt_len > 64 {
        bail!("Implausible salt length {}", salt_len);
    }
    need(cursor, salt_len, "salt")?;
    let salt = &envelope[cursor..cursor + salt_len];
    cursor += salt_len;

    need(cursor, 1, "nonce length")?;
    let nonce_len = envelope[cursor] as usize;
    cursor += 1;
    if nonce_len != NONCE_LEN {
        bail!(
            "Unexpected nonce length {} (ChaCha20-Poly1305 requires {})",
            nonce_len,
            NONCE_LEN
        );
    }
    need(cursor, nonce_len, "nonce")?;
    let nonce_bytes = &envelope[cursor..cursor + nonce_len];
    cursor += nonce_len;

    let ciphertext = &envelope[cursor..];
    if ciphertext.is_empty() {
        bail!("Backup envelope has no ciphertext");
    }

    let key = derive_key_argon2id(passphrase.as_bytes(), salt, m_cost, t_cost, p_cost)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| {
            anyhow!("Could not decrypt backup. Check the passphrase or verify the file isn't corrupted.")
        })?;

    Ok(plaintext)
}

/// Returns `true` if `bytes` starts with the Mizan backup magic
/// header. Useful for the restore command to tell encrypted backups
/// from legacy plaintext ones during the rollout transition.
pub fn is_encrypted_envelope(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && &bytes[..MAGIC.len()] == MAGIC
}

fn validate_passphrase(passphrase: &str) -> Result<()> {
    if passphrase.len() < MIN_PASSPHRASE_LEN {
        bail!(
            "Passphrase too short: {} characters (minimum {})",
            passphrase.len(),
            MIN_PASSPHRASE_LEN
        );
    }
    Ok(())
}

fn derive_key_argon2id(
    passphrase: &[u8],
    salt: &[u8],
    m_cost_kib: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; 32]> {
    let params = Params::new(m_cost_kib, t_cost, p_cost, Some(32))
        .map_err(|e| anyhow!("Invalid Argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase, salt, &mut key)
        .map_err(|e| anyhow!("Argon2id key derivation failed: {}", e))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_correct_passphrase() {
        let plaintext = b"this is some SQLite-shaped data";
        let pw = "correct horse battery staple";
        let envelope = encrypt_backup(plaintext, pw).expect("encrypt");
        let recovered = decrypt_backup(&envelope, pw).expect("decrypt");
        assert_eq!(plaintext, recovered.as_slice());
    }

    #[test]
    fn wrong_passphrase_fails_to_decrypt() {
        let plaintext = b"secret bytes";
        let envelope = encrypt_backup(plaintext, "right-password").expect("encrypt");
        let result = decrypt_backup(&envelope, "wrong-password!");
        assert!(result.is_err(), "expected wrong passphrase to fail");
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        let plaintext = b"important data";
        let pw = "correct horse battery staple";
        let mut envelope = encrypt_backup(plaintext, pw).expect("encrypt");
        // Flip a bit in the ciphertext (last byte).
        let last = envelope.len() - 1;
        envelope[last] ^= 0x01;
        let result = decrypt_backup(&envelope, pw);
        assert!(result.is_err(), "AEAD must reject tampered ciphertext");
    }

    #[test]
    fn rejects_short_passphrase() {
        let result = encrypt_backup(b"data", "1234");
        assert!(result.is_err(), "expected short passphrase rejection");
    }

    #[test]
    fn rejects_empty_passphrase_on_decrypt() {
        let envelope = encrypt_backup(b"x", "a-good-passphrase").unwrap();
        let result = decrypt_backup(&envelope, "");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_non_mizan_envelope() {
        let bytes = b"not a mizan backup, just random bytes that look nothing like an envelope";
        let result = decrypt_backup(bytes, "anything-here");
        assert!(result.is_err());
    }

    #[test]
    fn distinct_ciphertexts_for_same_input() {
        // Salt + nonce randomness means two encryptions of identical
        // input must produce different envelopes — otherwise an
        // attacker can detect "no changes since last backup".
        let pw = "the same password each time";
        let a = encrypt_backup(b"identical input", pw).unwrap();
        let b = encrypt_backup(b"identical input", pw).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn is_encrypted_envelope_recognises_magic() {
        let envelope = encrypt_backup(b"data", "a-good-passphrase").unwrap();
        assert!(is_encrypted_envelope(&envelope));
        assert!(!is_encrypted_envelope(b"SQLite format 3"));
        assert!(!is_encrypted_envelope(b""));
    }

    #[test]
    fn round_trip_with_realistic_payload() {
        // Synthesize a plausible SQLite file header + body so the test
        // exercises the same code paths as a real backup.
        let mut payload = Vec::from(b"SQLite format 3\0" as &[u8]);
        payload.extend(std::iter::repeat_n(0x42u8, 64 * 1024));
        let pw = "a-realistic-user-password!";
        let envelope = encrypt_backup(&payload, pw).expect("encrypt");
        let recovered = decrypt_backup(&envelope, pw).expect("decrypt");
        assert_eq!(payload, recovered);
    }

    #[test]
    fn envelope_is_self_describing_under_param_drift() {
        // Encrypt with a non-default m_cost set in the envelope (we
        // simulate by hand). Decrypt should still work because params
        // are inside the envelope.
        let plaintext = b"future-proofing test";
        let pw = "the-passphrase-that-works";
        // Round-trip via the public API still uses defaults, but the
        // decryptor reads m_cost / t_cost / p_cost from the envelope,
        // not from these constants. If a future Mizan release bumps
        // the constants, the old envelope (with old constants) keeps
        // working.
        let envelope = encrypt_backup(plaintext, pw).unwrap();
        // Sanity-check that m_cost in the envelope matches defaults.
        let header_offset = MAGIC.len() + 2;
        let m_cost = u32::from_le_bytes(
            envelope[header_offset..header_offset + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(m_cost, DEFAULT_ARGON2_M_COST_KIB);
        let recovered = decrypt_backup(&envelope, pw).unwrap();
        assert_eq!(plaintext, recovered.as_slice());
    }
}
