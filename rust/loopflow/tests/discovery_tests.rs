use std::env;
use std::fs;
use std::sync::{Mutex, OnceLock};

use loopflow::builtins::{
    builtin_graph_names, builtin_skill_description, builtin_skill_names, BUILTIN_GRAPH_CATEGORIES,
    BUILTIN_SKILL_CATEGORIES,
};
use loopflow::definition::{resolve_definition, Target};
use loopflow::graph::load_graph;
use loopflow::graph::load_skill;
use loopflow::skills::catalog::SkillCatalog;
use tempfile::TempDir;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct HomeGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
    previous_home: Option<String>,
    provider_homes: Vec<(&'static str, Option<std::ffi::OsString>)>,
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
        let provider_homes = ["CODEX_HOME", "CLAUDE_CONFIG_DIR"]
            .into_iter()
            .map(|name| {
                let previous = env::var_os(name);
                env::remove_var(name);
                (name, previous)
            })
            .collect();
        Self {
            provider_homes,
            _lock: lock,
            previous_home,
            _temp: temp,
        }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        for (name, previous) in &self.provider_homes {
            match previous {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
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
        assert!(catalog.resolve(skill).unwrap().unwrap().path.is_none());
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
    assert!(builtin_graph_names()
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
            catalog.resolve(name).unwrap().unwrap().path,
            Some(skills_dir.join(format!("{name}.md")))
        );
    }
}

#[test]
fn discover_repo_graphs() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let graphs_dir = repo.path().join(".lf/graphs");
    std::fs::create_dir_all(&graphs_dir).expect("create graphs dir");
    std::fs::write(graphs_dir.join("ship.yaml"), "- implement\n- gate\n").expect("write graph");

    let graphs =
        loopflow::lf::commands::list::list_children(&["graph".to_string()], repo.path()).unwrap();
    let graph = graphs.iter().find(|f| f.name == "ship").expect("graph");
    assert_eq!(graph.description, "implement → gate");
}

#[test]
fn discover_namespaced_graphs_with_slash_names_and_authored_branch_summaries() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let graphs_dir = repo.path().join(".lf/graphs/gstack");
    let skills_dir = repo.path().join(".lf/skills/gstack");
    std::fs::create_dir_all(&graphs_dir).expect("create namespaced graphs dir");
    std::fs::create_dir_all(&skills_dir).expect("create namespaced skills dir");
    for skill in ["office-hours", "autoplan", "pr-review"] {
        std::fs::write(
            skills_dir.join(format!("{skill}.md")),
            format!("Run {skill}."),
        )
        .expect("write namespaced skill");
    }
    std::fs::write(
        graphs_dir.join("plan-manual.yaml"),
        "- gstack/office-hours\n",
    )
    .expect("write nested graph");
    std::fs::write(
        graphs_dir.join("sprint.yaml"),
        r#"
- gstack/office-hours
- xor:
    router: gstack/office-hours
    paths:
      autoplan:
        skill: gstack/autoplan
        description: "Auto-plan with minimal interaction"
      manual:
        graph: gstack/plan-manual
        description: "Interactive planning"
- implement
- gstack/pr-review
"#,
    )
    .expect("write graph");

    let graphs =
        loopflow::lf::commands::list::list_children(&["graph".to_string()], repo.path()).unwrap();
    let graph = graphs
        .iter()
        .find(|f| f.name == "gstack/sprint")
        .expect("graph");
    assert_eq!(
        graph.description,
        "gstack/office-hours → xor[gstack/office-hours]{autoplan: gstack/autoplan | manual: gstack/plan-manual} → implement → gstack/pr-review"
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
        catalog.resolve("qa").unwrap().unwrap().path,
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
fn resolve_target_finds_graph() {
    let _home = HomeGuard::new();
    let repo = TempDir::new().expect("repo");
    let target =
        resolve_definition(repo.path(), "pursue", None).expect("should find builtin graph");
    assert!(matches!(target, Target::Graph(_)));
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

/// Every builtin graph file on disk must appear in exactly one category and
/// must load via `load_graph`.
#[test]
fn every_builtin_graph_is_categorized_and_loadable() {
    let tmp = TempDir::new().expect("tempdir");

    let categorized: std::collections::HashMap<&str, &str> = BUILTIN_GRAPH_CATEGORIES
        .iter()
        .flat_map(|(cat, names)| names.iter().map(move |n| (*n, *cat)))
        .collect();

    for name in builtin_graph_names() {
        assert!(
            categorized.contains_key(name),
            "builtin graph {name} is missing from BUILTIN_GRAPH_CATEGORIES",
        );
        load_graph(name, tmp.path())
            .unwrap_or_else(|err| panic!("builtin graph {name} failed to load: {err}"));
    }

    let known: std::collections::HashSet<&'static str> =
        builtin_graph_names().into_iter().collect();
    for (_cat, names) in BUILTIN_GRAPH_CATEGORIES {
        for name in *names {
            assert!(
                known.contains(*name),
                "category lists graph {name} but no matching builtin exists"
            );
        }
    }
}

#[test]
fn installed_skills_share_listing_execution_and_graph_resolution() {
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
        catalog.resolve("explain-code").unwrap().unwrap().path,
        Some(skills.join("explain-code/SKILL.md"))
    );
    assert!(catalog.resolve("design").unwrap().unwrap().path.is_none());
    assert_eq!(
        load_skill("explain-code", repo.path())
            .unwrap()
            .content
            .as_deref(),
        Some("Explain code.")
    );
    let graphs = repo.path().join(".lf/graphs");
    fs::create_dir_all(&graphs).unwrap();
    fs::write(graphs.join("explain-code.yaml"), "- missing-child-23952\n").unwrap();
    let error = resolve_definition(repo.path(), "explain-code", None).unwrap_err();
    assert!(error.to_string().contains("invalid graph"), "{error}");
    assert!(error.to_string().contains("missing-child-23952"), "{error}");
}
