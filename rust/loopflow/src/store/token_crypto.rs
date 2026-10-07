use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::Engine;
use once_cell::sync::OnceCell;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 12;
const KEYCHAIN_SERVICE: &str = "loopflow.provider-token-key";
const KEYCHAIN_ACCOUNT: &str = "default";

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
    let path = fallback_key_path();
    if let Some(value) = load_key_from_file()? {
        return parse_key(&value);
    }
    // An explicit file belongs to an isolated store; never inspect the OS keyring.
    let retained = if cfg!(test) || env_key_path_override().is_some() {
        None
    } else {
        match load_key_from_platform() {
            Ok(key) => key,
            Err(_) if !has_encrypted_tokens()? => {
                // No credential in this Machine depends on the inaccessible key.
                // Its first credential gets an independent file-backed key.
                None
            }
            Err(error) => return Err(error),
        }
    };
    let key = match retained {
        Some(value) => parse_key(&value)?,
        None => {
            if !cfg!(test) && has_encrypted_tokens()? {
                return Err(TokenCryptoError::KeyRetrieval("existing encrypted credentials have no readable key; restore the retained key file or unlock the original keyring".into()));
            }
            let mut key = [0u8; KEY_BYTES];
            OsRng.fill_bytes(&mut key);
            key
        }
    };
    let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(key);
    let parent = path
        .parent()
        .ok_or_else(|| TokenCryptoError::KeyRetrieval("key path has no parent".into()))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(encoded.as_bytes())?;
    temporary.as_file().sync_all()?;
    match temporary.persist_noclobber(&path) {
        Ok(_) => Ok(key),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            parse_key(&std::fs::read_to_string(path)?)
        }
        Err(error) => Err(TokenCryptoError::Io(error.error)),
    }
}

fn has_encrypted_tokens() -> Result<bool, TokenCryptoError> {
    let path = crate::store::lf_home_dir().join("loopflow.db");
    if !path.exists() {
        return Ok(false);
    }
    let connection =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| {
                TokenCryptoError::KeyRetrieval(
                    "could not inspect existing encrypted credentials".into(),
                )
            })?;
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_tokens WHERE encrypted = 1)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| {
            TokenCryptoError::KeyRetrieval(
                "could not inspect existing encrypted credentials".into(),
            )
        })
}

fn load_key_from_platform() -> Result<Option<String>, TokenCryptoError> {
    #[cfg(target_os = "macos")]
    {
        load_key_from_macos_keychain()
    }
    #[cfg(target_os = "linux")]
    {
        load_key_from_secret_tool()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Ok(None)
    }
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
        // errSecItemNotFound (-25300) is security(1)'s exit 44. Locked,
        // denied or unavailable Keychain is unknown, never an absent key.
        if output.status.code() == Some(44) {
            return Ok(None);
        }
        return Err(TokenCryptoError::KeyRetrieval(
            "Keychain unavailable; unlock it once to retain the existing token key in its private file".into(),
        ));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        Err(TokenCryptoError::InvalidKey("stored key is empty".into()))
    } else {
        Ok(Some(value))
    }
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
        Err(TokenCryptoError::InvalidKey("stored key is empty".into()))
    } else {
        Ok(Some(value))
    }
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
        Err(TokenCryptoError::InvalidKey("stored key is empty".into()))
    } else {
        Ok(Some(value))
    }
}

#[cfg(test)]
fn fallback_key_path() -> PathBuf {
    env_key_path_override()
        .unwrap_or_else(|| std::env::temp_dir().join("loopflow-provider-token.key"))
}

#[cfg(not(test))]
fn fallback_key_path() -> PathBuf {
    env_key_path_override()
        .unwrap_or_else(|| crate::store::lf_home_dir().join("provider-token.key"))
}

fn env_key_path_override() -> Option<PathBuf> {
    let path = std::env::var("LF_PROVIDER_TOKEN_KEY_PATH").ok()?;
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
    use super::{decrypt_token, encrypt_token, load_or_create_key, parse_key, KEY_BYTES};
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
    #[test]
    fn private_file_key_survives_reload_and_invalid_bytes_are_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let _lock = crate::journal::test_env_lock();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("key");
        let previous = std::env::var_os("LF_PROVIDER_TOKEN_KEY_PATH");
        std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", &path);
        let key = load_or_create_key().unwrap();
        assert_eq!(load_or_create_key().unwrap(), key);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::write(&path, "unreadable-key").unwrap();
        assert!(load_or_create_key().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "unreadable-key");
        match previous {
            Some(value) => std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", value),
            None => std::env::remove_var("LF_PROVIDER_TOKEN_KEY_PATH"),
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn locked_keychain_is_unknown_and_only_item_not_found_is_absence() {
        use std::os::unix::fs::PermissionsExt;
        let _lock = crate::journal::test_env_lock();
        let directory = tempfile::tempdir().unwrap();
        let security = directory.path().join("security");
        let previous = std::env::var_os("PATH");
        std::env::set_var("PATH", directory.path());
        std::fs::write(&security, "#!/bin/sh\nexit 36\n").unwrap();
        std::fs::set_permissions(&security, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(super::load_key_from_macos_keychain().is_err());
        std::fs::write(&security, "#!/bin/sh\nexit 44\n").unwrap();
        assert_eq!(super::load_key_from_macos_keychain().unwrap(), None);
        match previous {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }
    }
}
