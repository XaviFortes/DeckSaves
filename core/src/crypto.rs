use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit},
    Aes256Gcm, Key, Nonce
};
use base64::{Engine as _, engine::general_purpose};
use anyhow::{Result, anyhow};
use sha2::{Sha256, Digest};

pub struct CredentialCrypto {
    cipher: Aes256Gcm,
}

impl CredentialCrypto {
    /// Create a new crypto instance with a key derived from the machine
    pub fn new() -> Result<Self> {
        let key = Self::derive_machine_key()?;
        let cipher = Aes256Gcm::new(&key);
        Ok(Self { cipher })
    }

    /// Encrypt a credential string
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        if plaintext.is_empty() {
            return Ok(String::new());
        }

        let nonce = Aes256Gcm::generate_nonce(&mut rand::thread_rng());
        let ciphertext = self.cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        // Combine nonce + ciphertext and encode as base64
        let mut combined = nonce.to_vec();
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(&combined))
    }

    /// Decrypt a credential string
    pub fn decrypt(&self, ciphertext: &str) -> Result<String> {
        if ciphertext.is_empty() {
            return Ok(String::new());
        }

        let combined = general_purpose::STANDARD
            .decode(ciphertext)
            .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;

        if combined.len() < 12 {
            return Err(anyhow!("Invalid ciphertext length"));
        }

        let (nonce_bytes, ciphertext_bytes) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext_bytes)
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        String::from_utf8(plaintext)
            .map_err(|e| anyhow!("UTF-8 conversion failed: {}", e))
    }

    /// Derive a machine-specific key for encryption
    fn derive_machine_key() -> Result<Key<Aes256Gcm>> {
        // Use hostname and user as seed for machine-specific key
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        // Create a deterministic key from machine info
        let mut hasher = Sha256::default();
        hasher.update(b"decksaves_crypto_v1");
        hasher.update(hostname.as_bytes());
        hasher.update(username.as_bytes());
        
        let key_bytes = hasher.finalize();
        Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let crypto = CredentialCrypto::new().unwrap();
        let plaintext = "test_secret_key";
        
        let encrypted = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_empty_string() {
        let crypto = CredentialCrypto::new().unwrap();
        let encrypted = crypto.encrypt("").unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        
        assert_eq!("", decrypted);
    }
}
