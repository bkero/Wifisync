//! End-to-end encryption helpers for sync
//!
//! Implements the key derivation hierarchy:
//! ```text
//! Master Password (never transmitted)
//!     │
//!     └─[Argon2id]─► Master Key
//!                       │
//!                       ├─[HKDF "auth"]─► Auth Key (server authentication)
//!                       │
//!                       └─[HKDF "encrypt"]─► Encryption Key (E2E encryption)
//! ```

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use wifisync_sync_protocol::ChangePayload;

use crate::error::{Error, Result};

/// Size of the nonce in bytes
const NONCE_SIZE: usize = 12;
/// Size of derived keys
const KEY_SIZE: usize = 32;
/// Info string for auth key derivation
const AUTH_KEY_INFO: &[u8] = b"wifisync-auth-key";
/// Info string for encryption key derivation
const ENCRYPT_KEY_INFO: &[u8] = b"wifisync-encrypt-key";

/// Sync encryption helper
///
/// Provides end-to-end encryption so the server never sees plaintext credentials.
pub struct SyncEncryption {
    /// Key for authenticating with the server (used to derive auth_proof)
    auth_key: [u8; KEY_SIZE],
    /// Key for encrypting credential payloads
    encrypt_key: [u8; KEY_SIZE],
}

impl SyncEncryption {
    /// Create a new sync encryption helper from master password
    ///
    /// The master password is used to derive:
    /// 1. A master key via Argon2id
    /// 2. An auth key via HKDF for server authentication
    /// 3. An encryption key via HKDF for E2E encryption
    pub fn from_password(password: &str, salt: &[u8]) -> Result<Self> {
        // Derive master key using Argon2id
        let master_key = derive_master_key(password, salt)?;

        // Derive auth key using HKDF
        let auth_key = derive_subkey(&master_key, AUTH_KEY_INFO)?;

        // Derive encryption key using HKDF
        let encrypt_key = derive_subkey(&master_key, ENCRYPT_KEY_INFO)?;

        Ok(Self {
            auth_key,
            encrypt_key,
        })
    }

    /// Get the auth proof to send to server
    ///
    /// This is derived from the auth key and can be verified by the server
    /// without revealing the master password.
    #[must_use]
    pub fn auth_proof(&self) -> String {
        // Return base64-encoded auth key as the proof
        // The server will hash this with bcrypt for storage
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        URL_SAFE_NO_PAD.encode(self.auth_key)
    }

    /// Encrypt a credential payload for sync
    pub fn encrypt_payload(&self, plaintext: &[u8]) -> Result<ChangePayload> {
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create cipher and encrypt
        let cipher = ChaCha20Poly1305::new_from_slice(&self.encrypt_key)
            .map_err(|e| Error::encryption(format!("Failed to create cipher: {e}")))?;

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| Error::encryption(format!("Encryption failed: {e}")))?;

        Ok(ChangePayload::new(ciphertext, nonce_bytes.to_vec()))
    }

    /// Decrypt a credential payload from sync
    pub fn decrypt_payload(&self, payload: &ChangePayload) -> Result<Vec<u8>> {
        if payload.is_empty() {
            return Ok(Vec::new());
        }

        // Create nonce from payload
        if payload.nonce.len() != NONCE_SIZE {
            return Err(Error::data_corrupted("Invalid nonce size"));
        }
        let nonce = Nonce::from_slice(&payload.nonce);

        // Create cipher and decrypt
        let cipher = ChaCha20Poly1305::new_from_slice(&self.encrypt_key)
            .map_err(|e| Error::encryption(format!("Failed to create cipher: {e}")))?;

        let plaintext = cipher
            .decrypt(nonce, payload.encrypted_data.as_ref())
            .map_err(|_| Error::InvalidPassword)?;

        Ok(plaintext)
    }

    /// Encrypt a string for sync
    pub fn encrypt_string(&self, plaintext: &str) -> Result<ChangePayload> {
        self.encrypt_payload(plaintext.as_bytes())
    }

    /// Decrypt a string from sync
    pub fn decrypt_string(&self, payload: &ChangePayload) -> Result<String> {
        let bytes = self.decrypt_payload(payload)?;
        String::from_utf8(bytes).map_err(|e| Error::data_corrupted(format!("Invalid UTF-8: {e}")))
    }
}

/// Derive master key from password using Argon2id
fn derive_master_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_SIZE]> {
    use argon2::{Algorithm, Argon2, Params, Version};

    // Use same parameters as local crypto.rs for consistency
    let params = Params::new(
        16 * 1024, // 16 MB memory
        3,         // 3 iterations
        1,         // 1 parallelism
        Some(KEY_SIZE),
    )
    .map_err(|e| Error::encryption(format!("Invalid Argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| Error::encryption(format!("Key derivation failed: {e}")))?;

    Ok(key)
}

/// Derive a subkey using HKDF
fn derive_subkey(master_key: &[u8; KEY_SIZE], info: &[u8]) -> Result<[u8; KEY_SIZE]> {
    let hkdf = Hkdf::<Sha256>::new(None, master_key);

    let mut subkey = [0u8; KEY_SIZE];
    hkdf.expand(info, &mut subkey)
        .map_err(|_| Error::encryption("HKDF expansion failed"))?;

    Ok(subkey)
}

/// Generate a random salt for key derivation
pub fn generate_salt() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_roundtrip() {
        let password = "test_password_123";
        let salt = generate_salt();

        let enc = SyncEncryption::from_password(password, &salt).unwrap();

        let plaintext = b"Hello, World! This is a secret credential.";
        let payload = enc.encrypt_payload(plaintext).unwrap();
        let decrypted = enc.decrypt_payload(&payload).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_different_passwords_different_keys() {
        let salt = generate_salt();

        let enc1 = SyncEncryption::from_password("password1", &salt).unwrap();
        let enc2 = SyncEncryption::from_password("password2", &salt).unwrap();

        assert_ne!(enc1.auth_proof(), enc2.auth_proof());
    }

    #[test]
    fn test_wrong_password_fails() {
        let salt = generate_salt();

        let enc1 = SyncEncryption::from_password("correct_password", &salt).unwrap();
        let enc2 = SyncEncryption::from_password("wrong_password", &salt).unwrap();

        let plaintext = b"Secret data";
        let payload = enc1.encrypt_payload(plaintext).unwrap();

        // Decrypting with wrong password should fail
        let result = enc2.decrypt_payload(&payload);
        assert!(result.is_err());
    }

    #[test]
    fn test_string_encryption() {
        let password = "my_password";
        let salt = generate_salt();

        let enc = SyncEncryption::from_password(password, &salt).unwrap();

        let plaintext = "This is a secret message!";
        let payload = enc.encrypt_string(plaintext).unwrap();
        let decrypted = enc.decrypt_string(&payload).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_empty_payload() {
        let password = "password";
        let salt = generate_salt();

        let enc = SyncEncryption::from_password(password, &salt).unwrap();

        let payload = ChangePayload::empty();
        let decrypted = enc.decrypt_payload(&payload).unwrap();

        assert!(decrypted.is_empty());
    }
}
