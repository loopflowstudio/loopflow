use std::env;
use std::sync::{Mutex, OnceLock};

use lf::discovery::{
    builtin_steps, list_all_steps, list_directions, list_flows_with_steps, BUILTIN_CATEGORIES,
};
use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct HomeGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_home: Option<String>,
    _temp: TempDir,
}

impl HomeGuard {
    fn new() -> Self {
        let lock = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let temp = TempDir::new().expect("temp home");
        let previous_home = env::var("HOME").ok();
        env::set_var("HOME", temp.path());
        Self {
            _lock: lock,
            previous_home,
            _temp: temp,
        }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.previous_home {
            env::set_var("HOME", prev);
        } else {
            env::remove_var("HOME");
        }
    }
}

#[test]
fn discover_builtin_steps() {
    let _home = HomeGuard::new();
    let builtins = builtin_steps();
    let (_user, _global, builtin_only, _skills) = list_all_steps(None);
    for step in builtins {
        assert!(builtin_only.contains(&step));
    }
}

#[test]
fn discover_repo_steps() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let steps_dir = repo.path().join(".lf/steps");
    std::fs::create_dir_all(&steps_dir).expect("create steps dir");
    std::fs::write(steps_dir.join("custom.md"), "# custom").expect("write step");

    let (user_steps, _global, _builtin_only, _skills) = list_all_steps(Some(repo.path()));
    assert!(user_steps.contains(&"custom".to_string()));
}

#[test]
fn discover_repo_flows() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let flows_dir = repo.path().join(".lf/flows");
    std::fs::create_dir_all(&flows_dir).expect("create flows dir");
    std::fs::write(
        flows_dir.join("ship.yaml"),
        "steps:\n  - implement\n  - gate\n",
    )
    .expect("write flow");

    let flows = list_flows_with_steps(repo.path());
    let flow = flows.iter().find(|f| f.name == "ship").expect("flow");
    assert_eq!(flow.step_names, vec!["implement", "gate"]);
}

#[test]
fn repo_step_shadows_builtin() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let steps_dir = repo.path().join(".lf/steps");
    std::fs::create_dir_all(&steps_dir).expect("create steps dir");
    std::fs::write(steps_dir.join("review.md"), "# review").expect("write step");

    let (user_steps, _global, builtin_only, _skills) = list_all_steps(Some(repo.path()));
    assert!(user_steps.contains(&"review".to_string()));
    assert!(!builtin_only.contains(&"review".to_string()));
}

#[test]
fn discover_directions() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let dir = repo.path().join(".lf/directions");
    std::fs::create_dir_all(&dir).expect("create directions dir");
    std::fs::write(dir.join("focus.md"), "Be focused.").expect("write direction");

    let directions = list_directions(Some(repo.path()));
    assert!(directions.contains(&"focus".to_string()));
    assert!(directions.contains(&"product-engineer".to_string()));
}

#[test]
fn categorized_listing_includes_known_steps() {
    let builtins = builtin_steps();
    for (_category, steps) in BUILTIN_CATEGORIES {
        for step in *steps {
            assert!(builtins.contains(*step));
        }
    }
}
