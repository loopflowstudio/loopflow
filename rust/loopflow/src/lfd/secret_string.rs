use serde::Deserialize;
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

/// Opaque UTF-8 secret string.
///
/// Debug and display are always redacted. Extract the plaintext only via
/// [`SecretString::expose_secret`].
#[derive(Clone, Default)]
pub struct SecretString {
    value: Vec<u8>,
}

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into().into_bytes(),
        }
    }

    pub fn expose_secret(&self) -> &str {
        std::str::from_utf8(&self.value).expect("SecretString always stores valid UTF-8")
    }

    pub fn constant_time_eq_str(&self, other: &str) -> bool {
        self.value.ct_eq(other.as_bytes()).into()
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        self.value.ct_eq(&other.value).into()
    }
}

impl Eq for SecretString {}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl std::fmt::Display for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::SecretString;

    #[test]
    fn debug_and_display_are_redacted() {
        let secret = SecretString::from("top-secret-token-123");
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(format!("{secret}"), "[REDACTED]");
    }

    #[test]
    fn expose_secret_returns_original_plaintext() {
        let secret = SecretString::from("token-abc");
        assert_eq!(secret.expose_secret(), "token-abc");
    }

    #[test]
    fn equality_is_constant_time_and_value_based() {
        let a = SecretString::from("same-token");
        let b = SecretString::from("same-token");
        let c = SecretString::from("different-token");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn constant_time_eq_str_matches_against_plaintext() {
        let secret = SecretString::from("abc123");
        assert!(secret.constant_time_eq_str("abc123"));
        assert!(!secret.constant_time_eq_str("abc124"));
    }
}
