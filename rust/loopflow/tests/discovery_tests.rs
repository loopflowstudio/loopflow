use std::env;
use std::fs;
use std::sync::{Mutex, OnceLock};

use loopflow::engine::builtins::{
    builtin_flow_names, builtin_skill_description, builtin_skill_names, BUILTIN_FLOW_CATEGORIES,
    BUILTIN_SKILL_CATEGORIES,
};
use loopflow::engine::target::{resolve_definition, Target};
use loopflow::engine::{load_flow, load_skill, skill_catalog::SkillCatalog};
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
fn discover_builtin_skills() {
    let _home = HomeGuard::new();
    let catalog = SkillCatalog::discover(None).unwrap();
    for skill in builtin_skill_names() {
        assert!(catalog.resolve(skill).unwrap().path.is_none());
    }
}

#[test]
fn builtin_catalog_uses_portable_dashes_and_never_slashes_or_underscores() {
    let skill_names = builtin_skill_names();
    for scope in ["repo", "wave", "task"] {
        for role in ["operate", "session"] {
            let name = format!("{scope}-{role}");
            assert!(skill_names.contains(&name.as_str()), "{name}");
        }
    }
    assert!(!skill_names.iter().any(|name| name.starts_with("project-")));
    assert!(skill_names.contains(&"implement"));
    assert!(skill_names.iter().all(|name| !name.contains(['_', '/'])));
    assert!(builtin_flow_names()
        .iter()
        .all(|name| !name.contains(['_', '/'])));
}

#[test]
fn discover_repo_skills() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let skills_dir = repo.path().join(".lf/skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(skills_dir.join("custom.md"), "# custom").expect("write skill");
    std::fs::create_dir_all(skills_dir.join("team")).expect("create skill namespace");
    std::fs::write(skills_dir.join("team/review.md"), "# review").expect("write namespaced skill");

    let catalog = SkillCatalog::discover(Some(repo.path())).unwrap();
    for name in ["custom", "team/review"] {
        assert_eq!(
            catalog.resolve(name).unwrap().path,
            Some(skills_dir.join(format!("{name}.md")))
        );
    }
}

#[test]
fn discover_repo_flows() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let flows_dir = repo.path().join(".lf/flows");
    std::fs::create_dir_all(&flows_dir).expect("create flows dir");
    std::fs::write(flows_dir.join("ship.yaml"), "- implement\n- gate\n").expect("write flow");

    let flows =
        loopflow::lf::commands::list::list_children(&["flow".to_string()], repo.path()).unwrap();
    let flow = flows.iter().find(|f| f.name == "ship").expect("flow");
    assert_eq!(flow.description, "implement → gate");
}

#[test]
fn discover_namespaced_flows_with_slash_names_and_authored_branch_summaries() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let flows_dir = repo.path().join(".lf/flows/gstack");
    let skills_dir = repo.path().join(".lf/skills/gstack");
    std::fs::create_dir_all(&flows_dir).expect("create namespaced flows dir");
    std::fs::create_dir_all(&skills_dir).expect("create namespaced skills dir");
    for skill in ["office-hours", "autoplan", "pr-review"] {
        std::fs::write(
            skills_dir.join(format!("{skill}.md")),
            format!("Run {skill}."),
        )
        .expect("write namespaced skill");
    }
    std::fs::write(
        flows_dir.join("plan-manual.yaml"),
        "- gstack/office-hours\n",
    )
    .expect("write nested flow");
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
- gstack/pr-review
"#,
    )
    .expect("write flow");

    let flows =
        loopflow::lf::commands::list::list_children(&["flow".to_string()], repo.path()).unwrap();
    let flow = flows
        .iter()
        .find(|f| f.name == "gstack-sprint")
        .expect("flow");
    assert_eq!(
        flow.description,
        "gstack-office-hours → xor[gstack-office-hours]{autoplan: gstack-autoplan | manual: gstack-plan-manual} → implement → gstack-pr-review"
    );
}

#[test]
fn repo_skill_shadows_builtin() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let skills_dir = repo.path().join(".lf/skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(skills_dir.join("qa.md"), "# qa").expect("write skill");

    let catalog = SkillCatalog::discover(Some(repo.path())).unwrap();
    assert_eq!(
        catalog.resolve("qa").unwrap().path,
        Some(skills_dir.join("qa.md"))
    );
}

#[test]
fn resolve_target_finds_skill() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = resolve_definition(repo.path(), "debug", None).expect("should find builtin skill");
    assert!(matches!(target, Target::Skill(_)));
}

#[test]
fn resolve_target_finds_flow() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target = resolve_definition(repo.path(), "pursue", None).expect("should find builtin flow");
    assert!(matches!(target, Target::Flow(_)));
}

#[test]
fn resolve_target_errors_for_unknown() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let result = resolve_definition(repo.path(), "nonexistent", None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[test]
fn categorized_listing_includes_known_skills() {
    let builtins = builtin_skill_names();
    for (_category, skills) in BUILTIN_SKILL_CATEGORIES {
        for skill in *skills {
            assert!(
                builtins.contains(skill),
                "category includes unknown skill: {skill}"
            );
        }
    }
}

/// Every builtin skill file on disk must appear in exactly one category and
/// must resolve via `load_skill`. New files are picked up automatically by
/// `build.rs`; this test guards against a skill existing in the binary but not
/// in the list output.
#[test]
fn every_builtin_skill_is_categorized_and_discoverable() {
    let tmp = TempDir::new().expect("tempdir");

    let categorized: std::collections::HashMap<&str, &str> = BUILTIN_SKILL_CATEGORIES
        .iter()
        .flat_map(|(cat, names)| names.iter().map(move |n| (*n, *cat)))
        .collect();

    for name in builtin_skill_names() {
        // Appears in exactly one category.
        assert!(
            categorized.contains_key(name),
            "builtin skill {name} is missing from BUILTIN_SKILL_CATEGORIES",
        );

        // Resolves by exact name.
        let skill = load_skill(name, tmp.path())
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
    for (_cat, names) in BUILTIN_SKILL_CATEGORIES {
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

#[test]
fn installed_skills_share_listing_execution_and_flow_resolution() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().unwrap();
    let skills = repo.path().join(".agents/skills");
    for (name, body) in [
        ("explain-code", "Explain code."),
        ("design", "---\nloopflow: true\n---\nStale export"),
    ] {
        fs::create_dir_all(skills.join(name)).unwrap();
        fs::write(skills.join(name).join("SKILL.md"), body).unwrap();
    }
    let catalog = SkillCatalog::discover(Some(repo.path())).unwrap();
    assert_eq!(
        catalog.resolve("explain-code").unwrap().path,
        Some(skills.join("explain-code/SKILL.md"))
    );
    assert!(catalog.resolve("design").unwrap().path.is_none());
    assert_eq!(
        load_skill("explain-code", repo.path())
            .unwrap()
            .content
            .as_deref(),
        Some("Explain code.")
    );
    let flows = repo.path().join(".lf/flows");
    fs::create_dir_all(&flows).unwrap();
    fs::write(flows.join("explain-code.yaml"), "- missing-child-23952\n").unwrap();
    let error = resolve_definition(repo.path(), "explain-code", None).unwrap_err();
    assert!(error.to_string().contains("invalid flow"), "{error}");
    assert!(error.to_string().contains("missing-child-23952"), "{error}");
}
