use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub struct EnvGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_path: Option<String>,
    previous_home: Option<OsString>,
    _temp: TempDir,
    _home: Option<TempDir>,
}

impl EnvGuard {
    pub fn new(entries: &[(&str, &str)]) -> Self {
        Self::new_internal(entries, false)
    }

    #[allow(dead_code)] // Shared helper used only by tests that need HOME isolation.
    pub fn new_with_clean_home(entries: &[(&str, &str)]) -> Self {
        Self::new_internal(entries, true)
    }

    fn new_internal(entries: &[(&str, &str)], clean_home: bool) -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner());
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
        let previous_home = env::var_os("HOME");
        let home = if clean_home {
            let home = TempDir::new().expect("temp home dir");
            env::set_var("HOME", home.path());
            Some(home)
        } else {
            None
        };
        Self {
            _lock: lock,
            previous_path,
            previous_home,
            _temp: temp,
            _home: home,
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
        match &self.previous_home {
            Some(prev) => env::set_var("HOME", prev),
            None => env::remove_var("HOME"),
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
