use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use once_cell::sync::OnceCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const KEYCHAIN_SERVICE: &str = "studio.loopflow.lfd.provider-token-key";
const KEYCHAIN_ACCOUNT: &str = "default";
#[cfg(target_os = "linux")]
const SECRET_TOOL_LABEL: &str = "Loopflow LFD Provider Token Key";

static CACHED_KEY: OnceCell<[u8; KEY_BYTES]> = OnceCell::new();

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

pub fn encrypt_token(plaintext: &str) -> Result<String, TokenCryptoError> {
    let key = encryption_key()?;
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
    Ok(base64::engine::general_purpose::STANDARD_NO_PAD.encode(payload))
}

pub fn decrypt_token(ciphertext_b64: &str) -> Result<String, TokenCryptoError> {
    let payload = base64::engine::general_purpose::STANDARD_NO_PAD.decode(ciphertext_b64)?;
    if payload.len() <= NONCE_BYTES {
        return Err(TokenCryptoError::MalformedCiphertext);
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_BYTES);
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| TokenCryptoError::KeyRetrieval("invalid AES key length".to_string()))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| TokenCryptoError::Decrypt)?;
    String::from_utf8(plaintext).map_err(|_| TokenCryptoError::InvalidPlaintext)
}

pub fn decrypt_if_needed(value: &str, encrypted: bool) -> Result<String, TokenCryptoError> {
    if encrypted {
        decrypt_token(value)
    } else {
        Ok(value.to_string())
    }
}

pub fn encrypt_optional(token: Option<&str>) -> Result<Option<String>, TokenCryptoError> {
    token.map(encrypt_token).transpose()
}

fn encryption_key() -> Result<[u8; KEY_BYTES], TokenCryptoError> {
    CACHED_KEY.get_or_try_init(load_or_create_key).copied()
}

fn load_or_create_key() -> Result<[u8; KEY_BYTES], TokenCryptoError> {
    if let Some(value) = load_key_from_platform()? {
        return parse_key(&value);
    }

    let mut key = [0u8; KEY_BYTES];
    OsRng.fill_bytes(&mut key);
    let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(key);

    store_key_on_platform(&encoded)?;
    Ok(key)
}

fn load_key_from_platform() -> Result<Option<String>, TokenCryptoError> {
    #[cfg(target_os = "macos")]
    if let Some(value) = load_key_from_macos_keychain()? {
        return Ok(Some(value));
    }

    #[cfg(target_os = "linux")]
    if let Some(value) = load_key_from_secret_tool()? {
        return Ok(Some(value));
    }

    load_key_from_file()
}

fn store_key_on_platform(encoded: &str) -> Result<(), TokenCryptoError> {
    #[cfg(target_os = "macos")]
    if store_key_in_macos_keychain(encoded)? {
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    if store_key_in_secret_tool(encoded)? {
        return Ok(());
    }

    store_key_in_file(encoded)
}

#[cfg(target_os = "macos")]
fn load_key_from_macos_keychain() -> Result<Option<String>, TokenCryptoError> {
    let output = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
        ])
        .output()
        .map_err(TokenCryptoError::Io)?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

#[cfg(target_os = "macos")]
fn store_key_in_macos_keychain(encoded: &str) -> Result<bool, TokenCryptoError> {
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            KEYCHAIN_ACCOUNT,
            "-w",
            encoded,
        ])
        .status()
        .map_err(TokenCryptoError::Io)?;
    Ok(status.success())
}

#[cfg(target_os = "linux")]
fn load_key_from_secret_tool() -> Result<Option<String>, TokenCryptoError> {
    if !command_available("secret-tool") {
        return Ok(None);
    }
    let output = Command::new("secret-tool")
        .args([
            "lookup",
            "service",
            KEYCHAIN_SERVICE,
            "account",
            KEYCHAIN_ACCOUNT,
        ])
        .output()
        .map_err(TokenCryptoError::Io)?;
    if !output.status.success() {
        return Ok(None);
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

#[cfg(target_os = "linux")]
fn store_key_in_secret_tool(encoded: &str) -> Result<bool, TokenCryptoError> {
    if !command_available("secret-tool") {
        return Ok(false);
    }

    let mut child = Command::new("secret-tool")
        .args([
            "store",
            "--label",
            SECRET_TOOL_LABEL,
            "service",
            KEYCHAIN_SERVICE,
            "account",
            KEYCHAIN_ACCOUNT,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(TokenCryptoError::Io)?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(format!("{encoded}\n").as_bytes())
            .map_err(TokenCryptoError::Io)?;
    }

    let status = child.wait().map_err(TokenCryptoError::Io)?;
    Ok(status.success())
}

#[cfg(target_os = "linux")]
fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

fn load_key_from_file() -> Result<Option<String>, TokenCryptoError> {
    let path = fallback_key_path();
    if !path.exists() {
        return Ok(None);
    }
    let value = std::fs::read_to_string(path)?.trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn store_key_in_file(encoded: &str) -> Result<(), TokenCryptoError> {
    let path = fallback_key_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_with_private_permissions(&path, format!("{encoded}\n").as_bytes())?;
    Ok(())
}

fn write_with_private_permissions(path: &Path, content: &[u8]) -> Result<(), TokenCryptoError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content)?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
fn fallback_key_path() -> PathBuf {
    env_key_path_override()
        .unwrap_or_else(|| std::env::temp_dir().join("loopflow-provider-token.key"))
}

#[cfg(not(test))]
fn fallback_key_path() -> PathBuf {
    env_key_path_override().unwrap_or_else(|| crate::lfd::lf_home_dir().join("provider-token.key"))
}

fn env_key_path_override() -> Option<PathBuf> {
    let path = std::env::var("LFD_PROVIDER_TOKEN_KEY_PATH").ok()?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

fn parse_key(encoded: &str) -> Result<[u8; KEY_BYTES], TokenCryptoError> {
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD.decode(encoded.trim())?;
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
    use base64::Engine;

    #[test]
    fn encrypt_then_decrypt_round_trip() {
        let plaintext = "token-value-123";
        let ciphertext = encrypt_token(plaintext).expect("encrypt token");
        assert_ne!(ciphertext, plaintext);
        let decrypted = decrypt_token(&ciphertext).expect("decrypt token");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn parse_key_rejects_wrong_length() {
        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(vec![1u8; 8]);
        let err = parse_key(&encoded).expect_err("should fail");
        assert!(err
            .to_string()
            .contains(&format!("expected {KEY_BYTES} bytes")));
    }
}
