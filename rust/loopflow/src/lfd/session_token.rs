use rand::RngCore;
use std::path::PathBuf;

/// Generate a new session token and write it to disk.
pub fn generate_and_write() -> Result<String, std::io::Error> {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    let path = token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&path, &token)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

pub fn token_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("session-token")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct HomeGuard {
        original: Option<OsString>,
    }

    impl HomeGuard {
        fn snapshot() -> Self {
            Self {
                original: std::env::var_os("HOME"),
            }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                std::env::set_var("HOME", value);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn generate_and_write_persists_hex_token() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = HomeGuard::snapshot();
        let home = tempfile::tempdir().expect("tempdir should be created");
        std::env::set_var("HOME", home.path());

        let token = generate_and_write().expect("token should be generated");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));

        let persisted = std::fs::read_to_string(token_path()).expect("token file should exist");
        assert_eq!(persisted, token);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let perms = std::fs::metadata(token_path())
                .expect("metadata should exist")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(perms, 0o600);
        }
    }
}
