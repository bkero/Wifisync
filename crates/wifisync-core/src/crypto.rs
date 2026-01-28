//! Cryptographic operations for Wifisync
//!
//! This module provides encryption and decryption for credential storage and sharing.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

use crate::error::Error;
use crate::Result;

/// Size of the nonce in bytes
const NONCE_SIZE: usize = 12;
/// Size of the salt for key derivation
const SALT_SIZE: usize = 32;

/// Encrypted data container
#[derive(Debug, Clone)]
pub struct EncryptedData {
    /// Salt used for key derivation
    pub salt: Vec<u8>,
    /// Nonce used for encryption
    pub nonce: Vec<u8>,
    /// Encrypted ciphertext
    pub ciphertext: Vec<u8>,
}

impl EncryptedData {
    /// Serialize to bytes for storage
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(
            4 + self.salt.len() + 4 + self.nonce.len() + self.ciphertext.len(),
        );

        // Write salt length and salt
        result.extend_from_slice(&(self.salt.len() as u32).to_le_bytes());
        result.extend_from_slice(&self.salt);

        // Write nonce length and nonce
        result.extend_from_slice(&(self.nonce.len() as u32).to_le_bytes());
        result.extend_from_slice(&self.nonce);

        // Write ciphertext
        result.extend_from_slice(&self.ciphertext);

        result
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 8 {
            return Err(Error::data_corrupted("Encrypted data too short"));
        }

        let mut pos = 0;

        // Read salt
        let salt_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if data.len() < pos + salt_len + 4 {
            return Err(Error::data_corrupted("Invalid salt length"));
        }
        let salt = data[pos..pos + salt_len].to_vec();
        pos += salt_len;

        // Read nonce
        let nonce_len = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        if data.len() < pos + nonce_len {
            return Err(Error::data_corrupted("Invalid nonce length"));
        }
        let nonce = data[pos..pos + nonce_len].to_vec();
        pos += nonce_len;

        // Read ciphertext
        let ciphertext = data[pos..].to_vec();

        Ok(Self {
            salt,
            nonce,
            ciphertext,
        })
    }
}

/// Derive an encryption key from a password using Argon2id
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    use argon2::{Argon2, Params, Algorithm, Version};

    // Use Argon2id with reasonable parameters
    let params = Params::new(
        16 * 1024, // 16 MB memory
        3,         // 3 iterations
        1,         // 1 parallelism
        Some(32),  // 32 byte output
    )
    .map_err(|e| Error::encryption(format!("Invalid Argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| Error::encryption(format!("Key derivation failed: {e}")))?;

    Ok(key)
}

/// Base64 encode bytes (URL-safe, no padding)
fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

/// Encrypt data with a password
pub fn encrypt(plaintext: &[u8], password: &str) -> Result<EncryptedData> {
    // Generate random salt
    let mut salt = vec![0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);

    // Derive key
    let key = derive_key(password, &salt)?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|e| Error::encryption(format!("Failed to create cipher: {e}")))?;

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| Error::encryption(format!("Encryption failed: {e}")))?;

    Ok(EncryptedData {
        salt,
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypt data with a password
pub fn decrypt(encrypted: &EncryptedData, password: &str) -> Result<Vec<u8>> {
    // Derive key
    let key = derive_key(password, &encrypted.salt)?;

    // Create nonce
    let nonce = Nonce::from_slice(&encrypted.nonce);

    // Decrypt
    let cipher = ChaCha20Poly1305::new_from_slice(&key)
        .map_err(|e| Error::encryption(format!("Failed to create cipher: {e}")))?;

    let plaintext = cipher
        .decrypt(nonce, encrypted.ciphertext.as_ref())
        .map_err(|_| Error::InvalidPassword)?;

    Ok(plaintext)
}

/// Encrypt a string and return base64-encoded result
pub fn encrypt_string(plaintext: &str, password: &str) -> Result<String> {
    let encrypted = encrypt(plaintext.as_bytes(), password)?;
    Ok(base64_encode(&encrypted.to_bytes()))
}

/// Decrypt a base64-encoded encrypted string
pub fn decrypt_string(encrypted_b64: &str, password: &str) -> Result<String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let bytes = URL_SAFE_NO_PAD
        .decode(encrypted_b64)
        .map_err(|e| Error::data_corrupted(format!("Invalid base64: {e}")))?;

    let encrypted = EncryptedData::from_bytes(&bytes)?;
    let plaintext = decrypt(&encrypted, password)?;

    String::from_utf8(plaintext).map_err(|e| Error::data_corrupted(format!("Invalid UTF-8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let plaintext = b"Hello, World! This is a test message.";
        let password = "test_password_123";

        let encrypted = encrypt(plaintext, password).unwrap();
        let decrypted = decrypt(&encrypted, password).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_wrong_password() {
        let plaintext = b"Secret data";
        let encrypted = encrypt(plaintext, "correct_password").unwrap();

        let result = decrypt(&encrypted, "wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_data_serialization() {
        let plaintext = b"Test data";
        let password = "password";

        let encrypted = encrypt(plaintext, password).unwrap();
        let bytes = encrypted.to_bytes();
        let restored = EncryptedData::from_bytes(&bytes).unwrap();

        assert_eq!(encrypted.salt, restored.salt);
        assert_eq!(encrypted.nonce, restored.nonce);
        assert_eq!(encrypted.ciphertext, restored.ciphertext);

        // Should still decrypt
        let decrypted = decrypt(&restored, password).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_string_encryption() {
        let plaintext = "This is a secret message!";
        let password = "my_password";

        let encrypted = encrypt_string(plaintext, password).unwrap();
        let decrypted = decrypt_string(&encrypted, password).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
