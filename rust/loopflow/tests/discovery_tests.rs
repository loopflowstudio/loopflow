use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use loopflow::engine::builtins::{builtin_flow_names, builtin_skill_names};
use loopflow::engine::{load_flow, load_skill};
use loopflow::lf::discovery::{
    builtin_skill_description, builtin_skills, discover_skill, discover_target, list_all_skills,
    list_directions, list_user_flows, Target, BUILTIN_FLOW_CATEGORIES, BUILTIN_STEP_CATEGORIES,
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
fn discover_builtin_skills() {
    let _home = HomeGuard::new();
    let builtins = builtin_skills();
    let (_user, _global, builtin_only, _skills) = list_all_skills(None);
    for skill in builtins {
        assert!(builtin_only.contains(&skill));
    }
}

#[test]
fn discover_repo_skills() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let skills_dir = repo.path().join(".lf/skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(skills_dir.join("custom.md"), "# custom").expect("write skill");

    let (user_skills, _global, _builtin_only, _skills) = list_all_skills(Some(repo.path()));
    assert!(user_skills.contains(&"custom".to_string()));
}

#[test]
fn discover_repo_flows() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let flows_dir = repo.path().join(".lf/flows");
    std::fs::create_dir_all(&flows_dir).expect("create flows dir");
    std::fs::write(
        flows_dir.join("ship.yaml"),
        "skills:\n  - implement\n  - gate\n",
    )
    .expect("write flow");

    let flows = list_user_flows(repo.path());
    let flow = flows.iter().find(|f| f.name == "ship").expect("flow");
    assert_eq!(flow.skill_names, vec!["implement", "gate"]);
}

#[test]
fn discover_namespaced_flows_with_hyphenated_names_and_branch_summaries() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let flows_dir = repo.path().join(".lf/flows/gstack");
    std::fs::create_dir_all(&flows_dir).expect("create namespaced flows dir");
    std::fs::write(
        flows_dir.join("sprint.yaml"),
        r#"
- gstack/office-hours
- xor:
    router: gstack/office-hours
    paths:
      autoplan:
        skill: gstack/autoplan
        description: "Auto-plan with minimal interaction"
      manual:
        flow: gstack/plan-manual
        description: "Interactive planning"
- implement
- and:
    branches:
      - skill: gstack/pr-review
      - skill: gstack/cso
      - skill: gstack/codex
    synthesize: gstack/review-synthesize
"#,
    )
    .expect("write flow");

    let flows = list_user_flows(repo.path());
    let flow = flows
        .iter()
        .find(|f| f.name == "gstack-sprint")
        .expect("flow");
    assert_eq!(
        flow.skill_names,
        vec![
            "gstack/office-hours",
            "[xor]",
            "gstack/autoplan",
            "gstack/plan-manual",
            "implement",
            "[and]",
            "gstack/pr-review",
            "gstack/cso",
            "gstack/codex",
            "gstack/review-synthesize",
        ]
    );
}

#[test]
fn repo_skill_shadows_builtin() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let skills_dir = repo.path().join(".lf/skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(skills_dir.join("review.md"), "# review").expect("write skill");

    let (user_skills, _global, builtin_only, _skills) = list_all_skills(Some(repo.path()));
    assert!(user_skills.contains(&"review".to_string()));
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
fn discover_target_finds_skill() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = discover_target(repo.path(), "debug").expect("should find builtin skill");
    assert!(matches!(target, Target::Skill(_)));
}

#[test]
fn discover_target_finds_flow() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = discover_target(repo.path(), "build").expect("should find builtin flow");
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
fn categorized_listing_includes_known_skills() {
    let builtins = builtin_skills();
    for (_category, skills) in BUILTIN_STEP_CATEGORIES {
        for skill in *skills {
            assert!(
                builtins.contains(*skill),
                "category includes unknown skill: {skill}"
            );
        }
    }
}

/// Every builtin skill file on disk must appear in exactly one category and
/// must resolve via `discover_skill`. New files are picked up automatically by
/// `build.rs`; this test guards against a skill existing in the binary but not
/// in the list output.
#[test]
fn every_builtin_skill_is_categorized_and_discoverable() {
    let tmp = TempDir::new().expect("tempdir");

    let categorized: std::collections::HashMap<&str, &str> = BUILTIN_STEP_CATEGORIES
        .iter()
        .flat_map(|(cat, names)| names.iter().map(move |n| (*n, *cat)))
        .collect();

    for name in builtin_skill_names() {
        // Appears in exactly one category.
        assert!(
            categorized.contains_key(name),
            "builtin skill {name} is missing from BUILTIN_STEP_CATEGORIES",
        );

        // Resolves by exact name.
        let skill = discover_skill(tmp.path(), name)
            .unwrap_or_else(|err| panic!("builtin skill {name} did not resolve: {err}"));
        assert!(
            skill
                .content
                .as_deref()
                .map(|c| !c.is_empty())
                .unwrap_or(false),
            "builtin skill {name} loaded with empty content"
        );

        // Has a description (frontmatter or first prose line).
        let desc = builtin_skill_description(name);
        assert!(
            !desc.is_empty(),
            "builtin skill {name} has no description (add a `description:` frontmatter \
             field or a leading prose line)"
        );
    }

    // Every category entry must be a known builtin — no phantom names.
    let known: std::collections::HashSet<&'static str> =
        builtin_skill_names().into_iter().collect();
    for (_cat, names) in BUILTIN_STEP_CATEGORIES {
        for name in *names {
            assert!(
                known.contains(*name),
                "category lists {name} but no matching builtin exists"
            );
        }
    }
}

/// Every builtin flow file on disk must appear in exactly one category and
/// must load via `load_flow`.
#[test]
fn every_builtin_flow_is_categorized_and_loadable() {
    let tmp = TempDir::new().expect("tempdir");

    let categorized: std::collections::HashMap<&str, &str> = BUILTIN_FLOW_CATEGORIES
        .iter()
        .flat_map(|(cat, names)| names.iter().map(move |n| (*n, *cat)))
        .collect();

    for name in builtin_flow_names() {
        assert!(
            categorized.contains_key(name),
            "builtin flow {name} is missing from BUILTIN_FLOW_CATEGORIES",
        );
        load_flow(name, tmp.path())
            .unwrap_or_else(|err| panic!("builtin flow {name} failed to load: {err}"));
    }

    let known: std::collections::HashSet<&'static str> = builtin_flow_names().into_iter().collect();
    for (_cat, names) in BUILTIN_FLOW_CATEGORIES {
        for name in *names {
            assert!(
                known.contains(*name),
                "category lists flow {name} but no matching builtin exists"
            );
        }
    }
}

/// Bare-name fallback: a skill in a namespaced builtin must also resolve by its
/// short name when no core skill / other namespaced skill shares that short name.
#[test]
fn namespaced_skills_resolve_by_bare_name_when_unique() {
    let tmp = TempDir::new().expect("tempdir");
    // office-hours lives only in gstack; it must also work as a bare name.
    let bare = load_skill("office-hours", tmp.path()).expect("bare name resolves");
    let qualified = load_skill("gstack/office-hours", tmp.path()).expect("qualified resolves");
    assert_eq!(bare.name, qualified.name);
}

/// A bare name that matches a core builtin must resolve to the core one, not
/// to any namespaced sibling.
#[test]
fn bare_name_prefers_core_over_namespaced() {
    let tmp = TempDir::new().expect("tempdir");
    // `debug` exists in core (build/) and in gstack/. Bare name → core.
    let skill = discover_skill(tmp.path(), "debug").expect("debug resolves");
    assert_eq!(skill.name, "debug");
    match discover_target(tmp.path(), "debug").expect("debug resolves via target") {
        Target::Skill(s) => assert_eq!(s.name, "debug"),
        Target::Flow(f) => panic!("expected Skill, got Flow {}", f.name),
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

    let (_user, _global, _builtin, external) = list_all_skills(Some(repo.path()));
    assert!(external.contains(&("npx/explain-code".to_string(), "npx skills".to_string())));
    assert!(
        !external.iter().any(|(name, _)| name == "npx/design"),
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

    let skill = discover_skill(repo.path(), "npx/explain-code").expect("load npx skill");
    assert!(skill
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

    let skill = discover_skill(repo.path(), "npx/deep-research").expect("load npx skill");
    assert!(skill
        .content
        .as_deref()
        .is_some_and(|content| content.contains("Loaded from find fallback")));

    let trace = fs::read_to_string(&trace_file).expect("read trace file");
    assert!(trace.contains("skills add deep-research"));
    assert!(trace.contains("skills find deep-research"));
    assert!(trace.contains("skills add vercel-labs/deep-research"));
}

#[test]
fn npx_find_handles_qualified_skill_format() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let trace_file = repo.path().join("npx-qualified.log");
    let npx_script = repo.path().join("fake-npx-qualified.sh");

    // Simulate npx skills find returning owner/repo@skill with ANSI codes
    let script = format!(
        r#"#!/bin/sh
set -e
echo "$@" >> "{trace}"
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "add" ] && [ "$4" = "skill-creator" ]; then
  exit 1
fi
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "find" ] && [ "$4" = "skill-creator" ]; then
  printf '\033[38;5;145manthropics/skills@skill-creator\033[0m \033[36m50.4K installs\033[0m\n'
  exit 0
fi
if [ "$1" = "--yes" ] && [ "$2" = "skills" ] && [ "$3" = "add" ] && [ "$4" = "anthropics/skills@skill-creator" ]; then
  mkdir -p ".agents/skills/skill-creator"
  cat > ".agents/skills/skill-creator/SKILL.md" <<'EOF'
---
name: skill-creator
description: Create skills
---
Loaded via qualified format
EOF
  exit 0
fi
exit 1
"#,
        trace = trace_file.display()
    );
    write_executable(&npx_script, &script);

    let _npx_bin = EnvVarGuard::set("LF_NPX_BIN", npx_script.display().to_string());

    let skill = discover_skill(repo.path(), "npx/skill-creator").expect("load qualified npx skill");
    assert!(skill
        .content
        .as_deref()
        .is_some_and(|content| content.contains("Loaded via qualified format")));

    let trace = fs::read_to_string(&trace_file).expect("read trace file");
    assert!(trace.contains("skills add skill-creator"));
    assert!(trace.contains("skills find skill-creator"));
    assert!(trace.contains("skills add anthropics/skills@skill-creator"));
}
