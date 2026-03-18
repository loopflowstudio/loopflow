use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use tempfile::TempDir;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct HomeOverride {
    original: Option<OsString>,
    _temp: Option<TempDir>,
}

impl HomeOverride {
    fn new_temp() -> Self {
        let temp = TempDir::new().expect("temp home dir");
        let original = env::var_os("LF_HOME");
        env::set_var("LF_HOME", temp.path());
        Self {
            original,
            _temp: Some(temp),
        }
    }
}

impl Drop for HomeOverride {
    fn drop(&mut self) {
        match &self.original {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
    }
}

#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn with_clean_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let _home = HomeOverride::new_temp();
    f()
}

pub struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_path: Option<String>,
    previous_home: Option<String>,
    _temp: TempDir,
    _home: Option<HomeOverride>,
}

impl EnvGuard {
    #[allow(dead_code)] // Shared helper compiled into multiple test crates.
    pub fn new(entries: &[(&str, &str)]) -> Self {
        Self::with_home(entries, None)
    }

    #[allow(dead_code)] // Shared helper used only by tests that need HOME isolation.
    pub fn with_home(entries: &[(&str, &str)], home: Option<&Path>) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
        let temp = TempDir::new().expect("temp bin dir");
        for (name, content) in entries {
            write_executable(temp.path(), name, content);
        }
        let previous_path = env::var("PATH").ok();
        let new_path = match &previous_path {
            Some(prev) => format!("{}:{}", temp.path().display(), prev),
            None => temp.path().display().to_string(),
        };
        env::set_var("PATH", new_path);
        let previous_home = env::var("HOME").ok();
        if let Some(home) = home {
            env::set_var("HOME", home);
        }
        Self {
            _lock: lock,
            previous_path,
            previous_home,
            _temp: temp,
            _home: None,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.previous_path {
            env::set_var("PATH", prev);
        } else {
            env::remove_var("PATH");
        }
        if let Some(prev) = &self.previous_home {
            env::set_var("HOME", prev);
        } else {
            env::remove_var("HOME");
        }
    }
}

fn write_executable(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}
