use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::prelude::*;

pub const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;

pub type TokenKeySource = fn() -> Result<[u8; KEY_BYTES], TokenCryptoError>;

#[derive(Debug, thiserror::Error)]
pub enum TokenCryptoError {
    #[error("invalid key format: {0}")]
    InvalidKey(String),
    #[error("key retrieval failed: {0}")]
    KeyRetrieval(String),
    #[error("encrypt failed")]
    Encrypt,
    #[error("decrypt failed")]
    Decrypt,
    #[error("ciphertext is malformed")]
    MalformedCiphertext,
    #[error("plaintext is not UTF-8")]
    InvalidPlaintext,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

pub fn encrypt_token(
    plaintext: &str,
    key_source: TokenKeySource,
) -> Result<String, TokenCryptoError> {
    let key = key_source()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| TokenCryptoError::KeyRetrieval("invalid AES key length".to_string()))?;
    let mut nonce = [0u8; NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|_| TokenCryptoError::Encrypt)?;
    let mut payload = Vec::with_capacity(NONCE_BYTES + ciphertext.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ciphertext);
    Ok(BASE64_STANDARD_NO_PAD.encode(payload))
}

pub fn decrypt_token(
    ciphertext_b64: &str,
    key_source: TokenKeySource,
) -> Result<String, TokenCryptoError> {
    let payload = BASE64_STANDARD_NO_PAD.decode(ciphertext_b64)?;
    if payload.len() <= NONCE_BYTES {
        return Err(TokenCryptoError::MalformedCiphertext);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_BYTES);
    let key = key_source()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| TokenCryptoError::KeyRetrieval("invalid AES key length".to_string()))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| TokenCryptoError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| TokenCryptoError::InvalidPlaintext)
}

pub fn decrypt_if_needed(
    value: &str,
    encrypted: bool,
    key_source: TokenKeySource,
) -> Result<String, TokenCryptoError> {
    if encrypted {
        decrypt_token(value, key_source)
    } else {
        Ok(value.to_string())
    }
}

pub fn encrypt_optional(
    token: Option<&str>,
    key_source: TokenKeySource,
) -> Result<Option<String>, TokenCryptoError> {
    token
        .map(|token| encrypt_token(token, key_source))
        .transpose()
}

pub fn parse_key(encoded: &str) -> Result<[u8; KEY_BYTES], TokenCryptoError> {
    let bytes = BASE64_STANDARD_NO_PAD.decode(encoded.trim())?;
    if bytes.len() != KEY_BYTES {
        return Err(TokenCryptoError::InvalidKey(format!(
            "expected {KEY_BYTES} bytes, got {}",
            bytes.len()
        )));
    }
    let mut key = [0u8; KEY_BYTES];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::{decrypt_token, encrypt_token, parse_key, KEY_BYTES};
    use base64::prelude::*;

    #[test]
    fn encrypt_then_decrypt_round_trip() {
        let plaintext = "token-value-123";
        let ciphertext = encrypt_token(plaintext, || Ok([7; KEY_BYTES])).expect("encrypt token");
        assert_ne!(ciphertext, plaintext);
        let decrypted = decrypt_token(&ciphertext, || Ok([7; KEY_BYTES])).expect("decrypt token");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn parse_key_rejects_wrong_length() {
        let encoded = BASE64_STANDARD_NO_PAD.encode(vec![1u8; 8]);
        let err = parse_key(&encoded).expect_err("should fail");
        assert!(err
            .to_string()
            .contains(&format!("expected {KEY_BYTES} bytes")));
    }
}
