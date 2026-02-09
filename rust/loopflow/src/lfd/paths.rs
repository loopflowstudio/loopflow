use std::path::PathBuf;

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Root data directory: ~/.lf
pub fn data_dir() -> PathBuf {
    home_dir().join(".lf")
}

/// SQLite database path: ~/.lf/lfd.db
pub fn db_path() -> PathBuf {
    data_dir().join("lfd.db")
}

/// Agent output directory: ~/.lf/output
pub fn output_dir() -> PathBuf {
    data_dir().join("output")
}

/// Log directory (platform-specific).
///
/// - macOS: ~/Library/Logs/lfd
/// - Linux: ~/.cache/lfd/logs
pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home_dir().join("Library/Logs/lfd")
    }

    #[cfg(not(target_os = "macos"))]
    {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("lfd/logs")
    }
}

/// launchd plist path (macOS only).
#[cfg(target_os = "macos")]
pub fn plist_path() -> PathBuf {
    home_dir().join("Library/LaunchAgents/studio.loopflow.lfd.plist")
}

/// systemd user service path (Linux only).
#[cfg(target_os = "linux")]
pub fn service_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("systemd/user/lfd.service")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_ends_with_lf() {
        let dir = data_dir();
        assert!(dir.ends_with(".lf"), "expected .lf suffix, got {dir:?}");
    }

    #[test]
    fn db_path_is_under_data_dir() {
        let db = db_path();
        assert!(db.starts_with(data_dir()));
        assert_eq!(db.file_name().unwrap(), "lfd.db");
    }

    #[test]
    fn output_dir_is_under_data_dir() {
        let out = output_dir();
        assert!(out.starts_with(data_dir()));
        assert_eq!(out.file_name().unwrap(), "output");
    }

    #[test]
    fn log_dir_exists_on_platform() {
        let dir = log_dir();
        // Just verify it returns a non-empty path
        assert!(!dir.as_os_str().is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plist_path_in_launch_agents() {
        let path = plist_path();
        assert!(
            path.to_string_lossy().contains("Library/LaunchAgents"),
            "expected LaunchAgents, got {path:?}"
        );
        assert_eq!(path.file_name().unwrap(), "studio.loopflow.lfd.plist");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn service_path_in_systemd_user() {
        let path = service_path();
        assert!(
            path.to_string_lossy().contains("systemd/user"),
            "expected systemd/user, got {path:?}"
        );
        assert_eq!(path.file_name().unwrap(), "lfd.service");
    }
}
