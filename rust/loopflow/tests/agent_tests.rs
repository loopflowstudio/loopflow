use std::env;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::engine::agent::{launch_agent, LaunchConfig};
use loopflow::engine::error::CoreError;
use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct PathGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_path: Option<String>,
    _temp: TempDir,
}

impl PathGuard {
    fn new(entries: &[(&str, &str)]) -> Self {
        Self::with_existing(entries, true)
    }

    fn new_isolated(entries: &[(&str, &str)]) -> Self {
        Self::with_existing(entries, false)
    }

    fn with_existing(entries: &[(&str, &str)], include_existing: bool) -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let temp = TempDir::new().expect("temp dir");
        for (name, content) in entries {
            write_executable(temp.path(), name, content);
        }
        let previous_path = env::var("PATH").ok();
        let new_path = if include_existing {
            match &previous_path {
                Some(prev) => format!("{}:{}", temp.path().display(), prev),
                None => temp.path().display().to_string(),
            }
        } else {
            temp.path().display().to_string()
        };
        env::set_var("PATH", new_path);
        Self {
            _lock: lock,
            previous_path,
            _temp: temp,
        }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.previous_path {
            env::set_var("PATH", prev);
        } else {
            env::remove_var("PATH");
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

fn base_config() -> LaunchConfig {
    LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: false,
        cwd: None,
        context_file: None,
        ..Default::default()
    }
}

#[test]
fn launch_returns_exit_code() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\nexit 0\n")]);
    let result = launch_agent("claude", "prompt", &base_config()).expect("launch");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn launch_captures_stdout() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\necho hello\n")]);
    let result = launch_agent("claude", "prompt", &base_config()).expect("launch");
    assert!(result.stdout.contains("hello"));
}

#[test]
fn launch_captures_stderr() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\necho error 1>&2\n")]);
    let result = launch_agent("claude", "prompt", &base_config()).expect("launch");
    assert!(result.stderr.contains("error"));
}

#[test]
fn launch_nonzero_exit() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\nexit 7\n")]);
    let result = launch_agent("claude", "prompt", &base_config()).expect("launch");
    assert_eq!(result.exit_code, 7);
}

#[test]
fn launch_missing_binary_returns_error() {
    let _env = PathGuard::new_isolated(&[]);
    let result = launch_agent("claude", "prompt", &base_config());
    assert!(matches!(
        result,
        Err(CoreError::IoError(_)) | Err(CoreError::ExecutionFailed(_))
    ));
}

#[test]
fn launch_with_cwd() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\npwd\n")]);
    let cwd = TempDir::new().expect("cwd");
    let mut config = base_config();
    config.cwd = Some(cwd.path().to_path_buf());
    let result = launch_agent("claude", "prompt", &config).expect("launch");
    assert!(result.stdout.contains(&cwd.path().display().to_string()));
}

#[test]
fn launch_streaming_mode() {
    let _env = PathGuard::new(&[("claude", "#!/bin/sh\necho first\necho second\n")]);
    let mut config = base_config();
    config.stream = true;
    let result = launch_agent("claude", "prompt", &config).expect("launch");
    assert!(result.stdout.contains("first"));
    assert!(result.stdout.contains("second"));
}
