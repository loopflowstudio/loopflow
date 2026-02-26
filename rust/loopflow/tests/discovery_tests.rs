use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::lf::discovery::{
    builtin_steps, discover_step, discover_target, list_all_steps, list_directions,
    list_user_flows, Target, BUILTIN_CATEGORIES,
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

struct EnvVarGuard {
    key: String,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &str, value: impl Into<String>) -> Self {
        let previous = env::var(key).ok();
        env::set_var(key, value.into());
        Self {
            key: key.to_string(),
            previous,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            env::set_var(&self.key, previous);
        } else {
            env::remove_var(&self.key);
        }
    }
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).expect("write script");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
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

    let flows = list_user_flows(repo.path());
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
    let group_dir = dir.join("mygroup");
    std::fs::create_dir_all(&group_dir).expect("create group dir");
    std::fs::write(group_dir.join("alpha.md"), "Alpha").expect("write group direction");

    let directions = list_directions(Some(repo.path()));
    assert!(directions.contains(&"focus".to_string()));
    assert!(directions.contains(&"mygroup".to_string()));
    assert!(directions.contains(&"alpha".to_string()));
    assert!(directions.contains(&"security".to_string()));
    assert!(directions.contains(&"infra".to_string()));
}

#[test]
fn discover_target_finds_step() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = discover_target(repo.path(), "debug").expect("should find builtin step");
    assert!(matches!(target, Target::Step(_)));
}

#[test]
fn discover_target_finds_flow() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = discover_target(repo.path(), "ship").expect("should find builtin flow");
    assert!(matches!(target, Target::Flow(_)));
}

#[test]
fn discover_target_errors_for_unknown() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let result = discover_target(repo.path(), "nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
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

#[test]
fn npx_skills_are_listed_from_cache_and_loopflow_skipped() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let skills_dir = repo.path().join(".agents/skills");
    fs::create_dir_all(skills_dir.join("explain-code")).expect("create explain-code dir");
    fs::create_dir_all(skills_dir.join("design")).expect("create design dir");

    fs::write(
        skills_dir.join("explain-code/SKILL.md"),
        "---\nname: explain-code\ndescription: Explain code.\n---\nExplain code.",
    )
    .expect("write cached skill");
    fs::write(
        skills_dir.join("design/SKILL.md"),
        "---\nname: design\ndescription: built-in\nloopflow: true\n---\nInjected",
    )
    .expect("write loopflow marker skill");

    let (_user, _global, _builtin, external) = list_all_steps(Some(repo.path()));
    assert!(external.contains(&("npx:explain-code".to_string(), "npx skills".to_string())));
    assert!(
        !external.iter().any(|(name, _)| name == "npx:design"),
        "loopflow marker skills should be excluded from npx listing"
    );
}

#[test]
fn npx_cache_miss_runs_add_and_loads_skill() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let trace_file = repo.path().join("npx.log");
    let npx_script = repo.path().join("fake-npx-add.sh");

    let script = format!(
        r#"#!/bin/sh
set -e
echo "$@" >> "{trace}"
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "add" ] && [ "$4" = "explain-code" ]; then
  mkdir -p ".agents/skills/explain-code"
  cat > ".agents/skills/explain-code/SKILL.md" <<'EOF'
---
name: explain-code
description: Explain code
---
Loaded from add
EOF
  exit 0
fi
exit 1
"#,
        trace = trace_file.display()
    );
    write_executable(&npx_script, &script);

    let _npx_bin = EnvVarGuard::set("LF_NPX_BIN", npx_script.display().to_string());

    let step = discover_step(repo.path(), "npx:explain-code").expect("load npx skill");
    assert!(step
        .content
        .as_deref()
        .is_some_and(|content| content.contains("Loaded from add")));

    let trace = fs::read_to_string(&trace_file).expect("read trace file");
    assert!(trace.contains("skills add explain-code"));
}

#[test]
fn npx_find_fallback_runs_when_add_fails() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let trace_file = repo.path().join("npx-find.log");
    let npx_script = repo.path().join("fake-npx-find.sh");

    let script = format!(
        r#"#!/bin/sh
set -e
echo "$@" >> "{trace}"
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "add" ] && [ "$4" = "deep-research" ]; then
  exit 1
fi
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "find" ] && [ "$4" = "deep-research" ]; then
  echo "vercel-labs/deep-research"
  exit 0
fi
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "add" ] && [ "$4" = "vercel-labs/deep-research" ]; then
  mkdir -p ".agents/skills/deep-research"
  cat > ".agents/skills/deep-research/SKILL.md" <<'EOF'
---
name: deep-research
description: Deep research
---
Loaded from find fallback
EOF
  exit 0
fi
exit 1
"#,
        trace = trace_file.display()
    );
    write_executable(&npx_script, &script);

    let _npx_bin = EnvVarGuard::set("LF_NPX_BIN", npx_script.display().to_string());

    let step = discover_step(repo.path(), "npx:deep-research").expect("load npx skill");
    assert!(step
        .content
        .as_deref()
        .is_some_and(|content| content.contains("Loaded from find fallback")));

    let trace = fs::read_to_string(&trace_file).expect("read trace file");
    assert!(trace.contains("skills add deep-research"));
    assert!(trace.contains("skills find deep-research"));
    assert!(trace.contains("skills add vercel-labs/deep-research"));
}
