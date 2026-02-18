use std::path::{Path, PathBuf};

/// Generate a new session token and write it to disk.
pub fn generate_and_write() -> Result<String, std::io::Error> {
    generate_and_write_at(&token_path())
}

fn generate_and_write_at(path: &Path) -> Result<String, std::io::Error> {
    let mut bytes = [0_u8; 32];
    rand::fill(&mut bytes);
    let token = hex::encode(bytes);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, &token)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

pub fn token_path() -> PathBuf {
    crate::lfd::lf_home_dir().join("session-token")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_and_write_persists_hex_token() {
        let home = tempfile::tempdir().expect("tempdir should be created");
        let token_path = home.path().join(".lf").join("session-token");

        let token = generate_and_write_at(&token_path).expect("token should be generated");
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));

        let persisted = std::fs::read_to_string(&token_path).expect("token file should exist");
        assert_eq!(persisted, token);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let perms = std::fs::metadata(&token_path)
                .expect("metadata should exist")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(perms, 0o600);
        }
    }
}
