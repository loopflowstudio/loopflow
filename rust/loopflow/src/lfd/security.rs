use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PathSecurityError {
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("path escapes root: {0}")]
    PathEscapesRoot(String),
    #[error("invalid id: {0}")]
    InvalidId(String),
}

pub fn validate_safe_id(id: &str) -> Result<(), PathSecurityError> {
    if id.is_empty() {
        return Err(PathSecurityError::InvalidId(
            "id must not be empty".to_string(),
        ));
    }
    if id == "." || id == ".." {
        return Err(PathSecurityError::InvalidId(
            "id must not be '.' or '..'".to_string(),
        ));
    }
    if id.chars().all(is_safe_id_char) {
        Ok(())
    } else {
        Err(PathSecurityError::InvalidId(
            "id must contain only [A-Za-z0-9_-]".to_string(),
        ))
    }
}

pub fn sanitize_fs_component(value: &str) -> String {
    let mut sanitized = String::new();
    let mut pending_dash = false;

    for ch in value.chars() {
        if is_safe_id_char(ch) {
            if pending_dash && !sanitized.is_empty() {
                sanitized.push('-');
            }
            pending_dash = false;
            sanitized.push(ch);
        } else {
            pending_dash = true;
        }
    }

    if sanitized.is_empty() {
        sanitized = "wave".to_string();
    }

    validate_safe_id(&sanitized).expect("sanitize_fs_component always returns a safe id");
    sanitized
}

pub fn path_within_root_existing(
    root: &Path,
    candidate: &Path,
) -> Result<PathBuf, PathSecurityError> {
    let root = canonicalize_existing_path(root)?;
    validate_relative_candidate(candidate)?;

    let target = root.join(candidate);
    let canonical_target = canonicalize_existing_path(&target)?;
    ensure_within_root(&root, &canonical_target)?;
    Ok(canonical_target)
}

pub fn path_within_root_planned(
    root: &Path,
    candidate: &Path,
) -> Result<PathBuf, PathSecurityError> {
    let root = canonicalize_existing_path(root)?;
    validate_relative_candidate(candidate)?;

    let target = root.join(candidate);
    let parent = target.parent().ok_or_else(|| {
        PathSecurityError::InvalidPath("candidate must include a parent path".to_string())
    })?;
    let canonical_parent = canonicalize_existing_path(parent)?;
    ensure_within_root(&root, &canonical_parent)?;

    let file_name = target.file_name().ok_or_else(|| {
        PathSecurityError::InvalidPath("candidate must include a final component".to_string())
    })?;

    let planned = canonical_parent.join(file_name);
    ensure_within_root(&root, &planned)?;
    Ok(planned)
}

pub fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, PathSecurityError> {
    validate_raw_path(path)?;
    path.canonicalize().map_err(|err| {
        PathSecurityError::InvalidPath(format!(
            "failed to canonicalize '{}': {err}",
            path.display()
        ))
    })
}

fn validate_relative_candidate(path: &Path) -> Result<(), PathSecurityError> {
    validate_raw_path(path)?;
    if path.is_absolute() {
        return Err(PathSecurityError::InvalidPath(
            "absolute paths are not allowed".to_string(),
        ));
    }

    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err(PathSecurityError::InvalidPath(
                    "parent traversal is not allowed".to_string(),
                ));
            }
            Component::Prefix(_) => {
                return Err(PathSecurityError::InvalidPath(
                    "windows path prefixes are not allowed".to_string(),
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_raw_path(path: &Path) -> Result<(), PathSecurityError> {
    let raw = path.as_os_str().to_string_lossy();
    if raw.contains('\0') {
        return Err(PathSecurityError::InvalidPath(
            "null bytes are not allowed".to_string(),
        ));
    }
    if raw.chars().any(char::is_control) {
        return Err(PathSecurityError::InvalidPath(
            "control characters are not allowed".to_string(),
        ));
    }
    if has_windows_prefix(&raw) {
        return Err(PathSecurityError::InvalidPath(
            "windows path prefixes are not allowed".to_string(),
        ));
    }
    Ok(())
}

fn ensure_within_root(root: &Path, candidate: &Path) -> Result<(), PathSecurityError> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(PathSecurityError::PathEscapesRoot(format!(
            "{} is outside {}",
            candidate.display(),
            root.display()
        )))
    }
}

fn has_windows_prefix(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return true;
    }
    raw.starts_with("\\\\")
}

fn is_safe_id_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

#[cfg(test)]
mod tests {
    use super::{
        path_within_root_existing, path_within_root_planned, sanitize_fs_component,
        validate_safe_id,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn path_within_root_existing_rejects_parent_traversal() {
        let tmp = tempdir().expect("tempdir");
        let err = path_within_root_existing(tmp.path(), PathBuf::from("../escape").as_path())
            .unwrap_err();
        assert!(err.to_string().contains("parent traversal"));
    }

    #[test]
    fn path_within_root_existing_rejects_absolute_path() {
        let tmp = tempdir().expect("tempdir");
        let err = path_within_root_existing(tmp.path(), PathBuf::from("/tmp/escape").as_path())
            .unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[cfg(unix)]
    #[test]
    fn path_within_root_existing_rejects_symlink_escape() {
        let tmp = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside tempdir");
        let outside_file = outside.path().join("outside.txt");
        std::fs::write(&outside_file, "nope").expect("write outside file");
        let escape_link = tmp.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &escape_link).expect("create symlink");

        let err =
            path_within_root_existing(tmp.path(), PathBuf::from("escape/outside.txt").as_path())
                .unwrap_err();
        assert!(err.to_string().contains("escapes root"));
    }

    #[cfg(unix)]
    #[test]
    fn path_within_root_planned_rejects_symlink_escape() {
        let tmp = tempdir().expect("tempdir");
        let outside = tempdir().expect("outside tempdir");
        let escape_link = tmp.path().join("escape");
        std::os::unix::fs::symlink(outside.path(), &escape_link).expect("create symlink");

        let err = path_within_root_planned(tmp.path(), PathBuf::from("escape/new.log").as_path())
            .unwrap_err();
        assert!(err.to_string().contains("escapes root"));
    }

    #[test]
    fn path_within_root_planned_allows_non_existent_file_under_valid_parent() {
        let tmp = tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("logs")).expect("create logs dir");

        let planned = path_within_root_planned(tmp.path(), PathBuf::from("logs/new.log").as_path())
            .expect("planned path should resolve");
        let expected = tmp
            .path()
            .join("logs")
            .canonicalize()
            .expect("canonical logs dir")
            .join("new.log");
        assert_eq!(planned, expected);
        assert!(!planned.exists());
    }

    #[test]
    fn path_within_root_rejects_null_byte() {
        let tmp = tempdir().expect("tempdir");
        let err = path_within_root_planned(tmp.path(), PathBuf::from("bad\0name.log").as_path())
            .unwrap_err();
        assert!(err.to_string().contains("null bytes"));
    }

    #[test]
    fn validate_safe_id_accepts_alnum_dash_underscore() {
        assert!(validate_safe_id("abc-DEF_123").is_ok());
    }

    #[test]
    fn validate_safe_id_rejects_empty_or_separators() {
        assert!(validate_safe_id("").is_err());
        assert!(validate_safe_id("..").is_err());
        assert!(validate_safe_id("a/b").is_err());
        assert!(validate_safe_id("a\\b").is_err());
        assert!(validate_safe_id("a.b").is_err());
    }

    #[test]
    fn sanitize_fs_component_normalizes_to_safe_id() {
        assert_eq!(
            sanitize_fs_component("feature/new*wave"),
            "feature-new-wave"
        );
        assert_eq!(sanitize_fs_component("../.."), "wave");
    }
}
