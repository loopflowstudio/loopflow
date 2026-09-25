mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::engine::flow::{ConcreteStep, Skill, SkillStep, Step};
use loopflow::engine::{expand_flow, load_flow};
use support::codex_app_server_script;
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
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_BIN")
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env("NO_COLOR", "1")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_RUN_DIR")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_ACCOUNT_LEASE")
        .env_remove("LF_HUMAN_SESSION")
        .env_remove("LF_TRACE_ID")
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
"#,
    );

    let flow = load_flow("sample", repo).unwrap();
    assert_eq!(flow.name, "sample");
    assert_eq!(flow.items.len(), 2);
    assert_eq!(
        flow.items[0],
        Step::Skill(SkillStep {
            skill: Skill {
                name: "implement".to_string(),
                agent: None,
                default_agent: None,
                action_style: None,
                content: None,
            },
            policy: Default::default(),
        })
    );
    assert_eq!(
        flow.items[1],
        Step::Skill(SkillStep {
            skill: Skill {
                name: "review".to_string(),
                agent: None,
                default_agent: None,
                action_style: None,
                content: None,
            },
            policy: Default::default(),
        })
    );
}

#[test]
fn code_flow_records_each_skill_as_one_generic_run() {
    let repo = TempDir::new().unwrap();
    run_git(repo.path(), &["init", "-b", "main"]);
    run_git(repo.path(), &["config", "user.email", "test@example.com"]);
    run_git(repo.path(), &["config", "user.name", "Test"]);
    for skill in ["implement", "compress"] {
        write_skill(repo.path(), skill, &format!("Run the {skill} step."));
    }
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "fixture"]);

    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script("done", ""),
    );
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

    let output = run_lf(repo.path(), home.path(), &["runs", "--json"], None);
    assert!(
        output.status.success(),
        "lf runs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut skills = runs
        .iter()
        .filter_map(|run| run["skill"].as_str())
        .collect::<Vec<_>>();
    skills.sort_unstable();
    assert_eq!(skills, ["compress", "implement"]);
    assert!(runs.iter().all(|run| run["outcome"] == "completed"));
}

#[test]
fn observing_and_preparing_a_task_are_not_execution() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task = support::register_unrun_task(
        home.path(),
        repo.path(),
        "task-observation",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let starts = || {
        runtime
            .block_on(task.store.task_events_after(&task.task.id, 0))
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == loopflow::work::task::TaskEventKind::Started)
            .count()
    };
    // The shared evidence the desktop sidebar consumes.
    let started = || {
        runtime
            .block_on(task.store.task_started(&task.task.id))
            .unwrap()
    };
    assert_eq!(starts(), 0);
    assert!(!started(), "a prepared, unrun Task is not started");
    let read = run_lf(
        repo.path(),
        home.path(),
        &["runs", "--active", "--task", "INF-123", "--json"],
        None,
    );
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    assert_eq!(
        starts(),
        0,
        "a filtered active-Run read cannot start its Task"
    );

    write_skill(repo.path(), "review-proof", "Review the fixture.");
    write_flow(
        repo.path(),
        "review-first",
        "- step:\n    id: review\n    name: review-proof\n    human: true\n",
    );
    let prepared = run_lf(
        repo.path(),
        home.path(),
        &[
            "--task",
            "INF-123",
            "flow",
            "review-first",
            "-b",
            "--no-loopflow",
        ],
        None,
    );
    assert!(!prepared.status.success());
    assert!(
        String::from_utf8_lossy(&prepared.stderr).contains("waiting for human input"),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let sessions = run_lf(
        repo.path(),
        home.path(),
        &["session", "list", "--json"],
        None,
    );
    assert!(
        sessions.status.success(),
        "{}",
        String::from_utf8_lossy(&sessions.stderr)
    );
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&sessions.stdout).unwrap();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0]["run_id"].as_str().is_some());
    assert_eq!(
        starts(),
        0,
        "publishing and reading an unopened review only prepares its Run"
    );
    assert!(
        !started(),
        "an unopened review's prepared Run is not execution"
    );

    write_skill(repo.path(), "first-work", "Do this proof-owned work.");
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script("done", ""),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    for _ in 0..2 {
        let launched = run_lf(
            repo.path(),
            home.path(),
            &["--task", "INF-123", "first-work", "-b", "--no-loopflow"],
            Some(&path),
        );
        assert!(
            launched.status.success(),
            "{}",
            String::from_utf8_lossy(&launched.stderr)
        );
        assert_eq!(
            starts(),
            1,
            "independent execution records the existing Started event once"
        );
        assert!(started(), "a launched Run is durable start evidence");
    }
}

#[test]
fn bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone() {
    use loopflow::controller::wave::playhead::QueuedInvocation;
    use loopflow::durable::FlowPosition;
    use loopflow_test_support::TestRepo;

    let repo = TestRepo::new();
    let caller = TestRepo::new();
    let home = TempDir::new().unwrap();
    let task = support::register_unrun_task(
        home.path(),
        repo.path(),
        "task-contribution",
        &repo.head_sha(),
    );
    for skill in ["first", "second"] {
        write_skill(repo.path(), skill, &format!("Execute {skill}."));
    }
    write_flow(repo.path(), "contribution", "- first\n- second\n");
    // A real collision: bare and explicit skill must choose the skill, explicit
    // flow must execute both steps, including when Work-bound.
    write_skill(
        repo.path(),
        "contribution",
        "Execute the single contribution skill.",
    );
    fs::create_dir_all(repo.path().join("scratch")).unwrap();
    fs::write(
        repo.path().join("scratch/existing.md"),
        "Another contributor's unfinished work.",
    )
    .unwrap();
    let original_head = repo.head_sha();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let position = runtime
        .block_on(task.store.set_flow_position(
            &task.task.id,
            FlowPosition {
                task_id: task.task.id.clone(),
                invocation: QueuedInvocation::load(repo.path(), "code").unwrap(),
                session_run_id: None,
                ready_summary: None,
                cursor: loopflow::engine::ExecutionCursor {
                    index: 1,
                    iteration: 4,
                    ..Default::default()
                },
                version: 0,
                worker_generation: 0,
                claim: None,
                failure: None,
                updated_at: time::OffsetDateTime::now_utc(),
            },
        ))
        .unwrap();

    let bin = TempDir::new().unwrap();
    let provider = codex_app_server_script("done", "if [ \"$1\" = --version ]; then exit 0; fi\npwd >> \"$LF_CONTROL_HOME/cwds\"").replace(
        "read -r turn_start",
        "read -r turn_start\nprintf '%s\\n' \"$turn_start\" >> \"$LF_CONTROL_HOME/prompts\"\nprintf '%s\\n' 'Evidence from preceding step.' > scratch/step.md",
    );
    write_executable(&bin.path().join("codex"), &provider);
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    // Distinct name tests bare flow dispatch without the collision above.
    write_flow(repo.path(), "two-steps", "- first\n- second\n");
    for args in [
        vec!["--task", "INF-123", "two-steps"],
        vec!["--task", "INF-123", "flow", "contribution"],
        vec!["--as", "task:INF-123", "flow", "contribution"],
    ] {
        let _ = fs::remove_file(home.path().join("prompts"));
        let _ = fs::remove_file(home.path().join("cwds"));
        let _ = fs::remove_file(repo.path().join("scratch/step.md"));
        let mut args = args;
        args.extend(["-b", "--no-loopflow", "Keep the Task context."]);
        let output = run_lf(caller.path(), home.path(), &args, Some(&path));
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let prompts = fs::read_to_string(home.path().join("prompts")).unwrap();
        let prompts: Vec<_> = prompts.lines().collect();
        assert_eq!(
            prompts.len(),
            2,
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        for prompt in &prompts {
            assert!(prompt.contains("Exercise the persisted lifecycle."));
            assert!(prompt.contains(task.task.id.as_str()));
            assert!(prompt.contains("Another contributor's unfinished work."));
            assert!(prompt.contains("Keep the Task context."));
        }
        assert!(!prompts[0].contains("Evidence from preceding step."));
        assert!(prompts[1].contains("Evidence from preceding step."));
        let cwds = fs::read_to_string(home.path().join("cwds")).unwrap();
        for cwd in cwds.lines() {
            assert_eq!(
                Path::new(cwd).canonicalize().unwrap(),
                repo.path().canonicalize().unwrap()
            );
        }
        assert_eq!(repo.head_sha(), original_head);
        assert_eq!(
            runtime
                .block_on(task.store.flow_position(&task.task.id))
                .unwrap(),
            Some(position.clone())
        );
        let staged = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(staged.stdout.is_empty());
    }
    let output = run_lf(repo.path(), home.path(), &["runs", "--json"], None);
    assert!(output.status.success());
    let runs: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(runs.len(), 6);
    for run in runs {
        assert!(run["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|subject| subject["selector"] == format!("task:{}", task.task.plan.identifier)));
        assert_eq!(run["outcome"], "completed");
    }
    for invocation in [
        vec!["contribution"],
        vec!["skill", "contribution"],
        vec!["design"],
    ] {
        let _ = fs::remove_file(home.path().join("prompts"));
        let mut args = vec!["--task", "INF-123"];
        args.extend(invocation);
        args.extend(["-b", "--no-loopflow"]);
        let output = run_lf(caller.path(), home.path(), &args, Some(&path));
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(home.path().join("prompts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
    // A human boundary remains explicit and cannot silently run the next step
    // just because the invocation has Task attribution.
    write_flow(
        repo.path(),
        "review-contribution",
        "- step:\n    id: accept\n    name: first\n    human: true\n- second\n",
    );
    fs::remove_file(home.path().join("prompts")).unwrap();
    let output = run_lf(
        caller.path(),
        home.path(),
        &["--task", "INF-123", "review-contribution", "-b"],
        Some(&path),
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Flow is waiting for human input"));
    assert!(!home.path().join("prompts").exists());
    let listed = run_lf(
        repo.path(),
        home.path(),
        &["session", "list", "--all", "--json"],
        Some(&path),
    );
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let sessions: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    let session = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"].as_str().unwrap().starts_with("flow:"))
        .unwrap();
    assert_eq!(session["work"]["id"], task.task.id.to_string());
    assert_eq!(session["flow_membership"]["flow"], "review-contribution");
    assert_eq!(session["flow_membership"]["current"], true);
    let run_id = session["run_id"].as_str().unwrap();
    let renamed = run_lf(
        repo.path(),
        home.path(),
        &["session", "rename", run_id, "Contribution review", "--json"],
        Some(&path),
    );
    assert!(
        renamed.status.success(),
        "{}",
        String::from_utf8_lossy(&renamed.stderr)
    );
    let renamed: serde_json::Value = serde_json::from_slice(&renamed.stdout).unwrap();
    assert_eq!(renamed["id"], session["id"]);
    assert_eq!(renamed["title_source"], "human");
    let opened = run_lf(
        repo.path(),
        home.path(),
        &["session", "open", session["id"].as_str().unwrap(), "--json"],
        Some(&path),
    );
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let opened: serde_json::Value = serde_json::from_slice(&opened.stdout).unwrap();
    assert_eq!(opened["title"], "Contribution review");
    assert_eq!(opened["run_id"], run_id);
    assert_eq!(opened["work"], session["work"]);
    assert!(!home.path().join("prompts").exists());
    assert_eq!(
        runtime
            .block_on(task.store.flow_position(&task.task.id))
            .unwrap(),
        Some(position)
    );
    assert_eq!(repo.head_sha(), original_head);
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
- op: pr land
"#,
    );

    let flow = load_flow("ship-ish", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    match &flow.items[1] {
        Step::Op(item) => {
            assert_eq!(item.command, "pr");
            assert_eq!(item.args, vec!["land"]);
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

    write_skill(repo, "skill-a", "First captured skill.");
    write_skill(repo, "skill-b", "Second captured skill.");
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
