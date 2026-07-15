use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::engine::flow::{ConcreteStep, Skill, Step};
use loopflow::engine::{expand_flow, load_flow};
use loopflow::store::sqlite::SqliteStore;
use tempfile::TempDir;

fn write_skill(repo: &Path, name: &str, content: &str) {
    let skills_dir = repo.join(".lf/skills");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(skills_dir.join(format!("{name}.md")), content).unwrap();
}

fn write_flow(repo: &Path, name: &str, content: &str) {
    let flows_dir = repo.join(".lf/flows");
    fs::create_dir_all(&flows_dir).unwrap();
    fs::write(flows_dir.join(format!("{name}.yaml")), content).unwrap();
}

fn expand_named_flow(repo: &Path, name: &str) -> Vec<ConcreteStep> {
    let flow = load_flow(name, repo).unwrap();
    expand_flow(&flow, repo).unwrap()
}

fn assert_skill_name(item: &ConcreteStep, expected: &str) {
    match item {
        ConcreteStep::Skill(skill) => assert_eq!(skill.skill.name, expected),
        other => panic!("expected skill {expected}, got {other:?}"),
    }
}

fn assert_skill_sequence(items: &[ConcreteStep], expected: &[&str]) {
    assert_eq!(items.len(), expected.len());
    for (item, skill_name) in items.iter().zip(expected) {
        assert_skill_name(item, skill_name);
    }
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }
}

fn run_lf(repo: &Path, home: &Path, args: &[&str], path: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(args)
        .current_dir(repo)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("NO_COLOR", "1")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_PROCESS_ID");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command.output().unwrap()
}

#[test]
fn flow_parsing_parity() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "sample",
        r#"
- implement
- step:
    name: review
    interactive: true
    direction: [ux, security]
"#,
    );

    let flow = load_flow("sample", repo).unwrap();
    assert_eq!(flow.name, "sample");
    assert_eq!(flow.items.len(), 2);
    assert_eq!(
        flow.items[0],
        Step::Skill(Skill {
            name: "implement".to_string(),
            agent: None,
            default_agent: None,
            directions: vec![],
            action_style: None,
            interactive: None,
            content: None,
            fast_path: None,
        })
    );
    assert_eq!(
        flow.items[1],
        Step::Skill(Skill {
            name: "review".to_string(),
            agent: None,
            default_agent: None,
            directions: vec!["ux".to_string(), "security".to_string()],
            action_style: None,
            interactive: Some(true),
            content: None,
            fast_path: None,
        })
    );
}

#[test]
fn code_flow_records_each_agent_launch_in_one_trace() {
    let repo = TempDir::new().unwrap();
    run_git(repo.path(), &["init", "-b", "main"]);
    run_git(repo.path(), &["config", "user.email", "test@example.com"]);
    run_git(repo.path(), &["config", "user.name", "Test"]);
    for skill in ["implement", "compress", "lint", "gate"] {
        write_skill(repo.path(), skill, &format!("Run the {skill} step."));
    }
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "fixture"]);

    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    write_executable(&bin.path().join("claude"), "#!/bin/sh\nexit 0\n");
    let path = std::env::var("PATH")
        .map(|path| format!("{}:{path}", bin.path().display()))
        .unwrap_or_else(|_| bin.path().display().to_string());

    let output = run_lf(
        repo.path(),
        home.path(),
        &["code", "-b", "--no-loopflow"],
        Some(&path),
    );
    assert!(
        output.status.success(),
        "lf code failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let events = store.list_run_events_since(0).unwrap();
    let run_id = events
        .iter()
        .find(|event| event.node == "run" && event.event == "started")
        .map(|event| event.run_id.as_str())
        .expect("flow run event");
    let launches = store.agent_launches_matching(run_id).unwrap();

    assert_eq!(launches.len(), 4);
    assert!(launches.iter().all(|launch| launch.run_id == run_id));
    assert!(launches
        .iter()
        .all(|launch| launch.process_id == launches[0].process_id));
    assert!(launches
        .iter()
        .all(|launch| launch.capture_status == "complete"));
    assert_eq!(
        launches
            .iter()
            .map(|launch| launch.skill.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["implement", "compress", "lint", "gate"]
    );
    assert_eq!(
        launches
            .iter()
            .map(|launch| launch.id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert_eq!(
        store
            .agent_turns_for_launches(
                &launches
                    .iter()
                    .map(|launch| launch.id.clone())
                    .collect::<Vec<_>>(),
            )
            .unwrap()
            .len(),
        4
    );

    let trace = run_lf(
        repo.path(),
        home.path(),
        &["trace", &launches[0].process_id, "--json"],
        None,
    );
    assert!(
        trace.status.success(),
        "lf trace failed: {}",
        String::from_utf8_lossy(&trace.stderr)
    );
    let trace: serde_json::Value = serde_json::from_slice(&trace.stdout).unwrap();
    assert_eq!(
        trace["launches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|launch| launch["skill"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["implement", "compress", "lint", "gate"]
    );

    let doctor = run_lf(repo.path(), home.path(), &["doctor", "--json"], None);
    assert!(
        doctor.status.success(),
        "lf doctor failed: {}",
        String::from_utf8_lossy(&doctor.stderr)
    );
    let doctor: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    let capture = doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "capture")
        .expect("capture check");
    assert_eq!(capture["status"], "ok");
}

#[test]
fn flow_ref_parses_into_items() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- review
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    assert!(matches!(flow.items[0], Step::FlowRef(_)));
    assert!(matches!(flow.items[1], Step::Skill(_)));
}

#[test]
fn ops_item_parses_and_expands() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "ship-ish",
        r#"
- implement
- op: pr land --create-pr
"#,
    );

    let flow = load_flow("ship-ish", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    match &flow.items[1] {
        Step::Op(item) => {
            assert_eq!(item.command, "pr");
            assert_eq!(item.args, vec!["land", "--create-pr"]);
        }
        other => panic!("expected ops item, got {other:?}"),
    }

    let expanded = expand_flow(&flow, repo).unwrap();
    assert!(matches!(&expanded[1], ConcreteStep::Op(_)));
}

#[test]
fn expand_flow_tracks_parents() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "child",
        r#"
- implement
"#,
    );
    write_flow(
        repo,
        "parent",
        r#"
- flow: child
- review
"#,
    );

    let flow = load_flow("parent", repo).unwrap();
    let items = expand_flow(&flow, repo).unwrap();
    match &items[0] {
        ConcreteStep::Skill(skill) => {
            assert_eq!(skill.skill.name, "implement");
            assert_eq!(skill.flow_parents, vec!["parent", "child"]);
        }
        _ => panic!("expected expanded skill"),
    }
}

/// Plain string items in flow YAML that match a sub-flow name should be
/// expanded as sub-flows, not treated as skill names.
#[test]
fn expand_flow_resolves_plain_string_as_subflow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_flow(repo, "publish", "- skill-a\n- skill-b");
    write_flow(repo, "parent", "- review\n- publish");

    let items = expand_named_flow(repo, "parent");

    assert_eq!(items.len(), 3, "publish should expand into its sub-skills");
    assert_skill_name(&items[0], "review");
    match &items[1] {
        ConcreteStep::Skill(s) => {
            assert_eq!(s.skill.name, "skill-a");
            assert_eq!(s.flow_parents, vec!["parent", "publish"]);
        }
        _ => panic!("expected skill from publish sub-flow"),
    }
    match &items[2] {
        ConcreteStep::Skill(s) => {
            assert_eq!(s.skill.name, "skill-b");
            assert_eq!(s.flow_parents, vec!["parent", "publish"]);
        }
        _ => panic!("expected skill from publish sub-flow"),
    }
}

/// A plain string that is both a skill name AND a flow name should NOT
/// be expanded as a sub-flow (skill takes priority to avoid ambiguity).
#[test]
fn expand_flow_prefers_skill_over_single_skill_flow() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    write_skill(repo, "review", "Review the code.");
    write_flow(repo, "parent", "- review");

    let items = expand_named_flow(repo, "parent");

    assert_eq!(items.len(), 1);
    match &items[0] {
        ConcreteStep::Skill(s) => {
            assert_eq!(s.skill.name, "review");
            assert_eq!(s.flow_parents, vec!["parent"]);
        }
        _ => panic!("expected skill"),
    }
}

#[test]
fn builtin_deploy_uses_ops_land_item() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "deploy");
    assert!(!items.is_empty());
    assert!(matches!(&items[1], ConcreteStep::Op(_)));
}

#[test]
fn builtin_garden_flow_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "garden");

    // garden: scan, assess, xor(act, silence)
    assert_eq!(items.len(), 3);
    assert_skill_name(&items[0], "scan");
    assert_skill_name(&items[1], "assess");
    match &items[2] {
        ConcreteStep::Xor(xor_def) => {
            assert_eq!(xor_def.paths.len(), 2);
            assert!(xor_def.paths.contains_key("act"));
            assert!(xor_def.paths.contains_key("silence"));
        }
        other => panic!("expected Xor, got {other:?}"),
    }
}

#[test]
fn builtin_governance_flows_structure() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let cases = [
        ("govern-identity", ["s5-scan", "s5-assess", "mutate"]),
        ("govern-intelligence", ["s4-scan", "s4-assess", "mutate"]),
        ("govern-control", ["s3-scan", "s3-assess", "mutate"]),
        ("govern-coordination", ["s2-scan", "s2-assess", "mutate"]),
    ];

    for (flow_name, expected) in cases {
        let items = expand_named_flow(repo, flow_name);
        assert_skill_sequence(&items, &expected);
    }
}

#[test]
fn builtin_build_or_silent_has_xor_branch() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "build-or-silent");
    // xor(build, silence) — the roadmap decision, no local ingest
    assert_eq!(items.len(), 1);
    match &items[0] {
        ConcreteStep::Xor(xor_def) => {
            assert!(xor_def.paths.contains_key("build"));
            assert!(xor_def.paths.contains_key("silence"));
        }
        other => panic!("expected Xor in build-or-silent, got {other:?}"),
    };
}
