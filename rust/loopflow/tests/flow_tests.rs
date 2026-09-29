mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::durable::{FlowSession, TaskWorkerClaimOutcome, TaskWorkerOwner};
use loopflow::engine::flow::{ConcreteStep, Skill, SkillStep, Step};
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::engine::{expand_flow, load_flow};
use loopflow::id::{ExecId, TraceId};
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

#[test]
fn mechanical_flow_boundaries_belong_to_flow_history_and_one_actual_exec() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    write_flow(
        repo.path(),
        "mechanical-proof",
        "- op: rebase --plan\n- op: rebase --plan\n",
    );
    let output = run_lf(
        repo.path(),
        home.path(),
        &["flow", "mechanical-proof", "-b", "--no-loopflow"],
        None,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let counts: (i64, i64, i64) = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='runs'), (SELECT COUNT(*) FROM agent_sessions), (SELECT COUNT(*) FROM execs)",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(
        counts,
        (0, 0, 1),
        "in-process operations create no Run, conversation or synthetic Exec"
    );
    let history: Vec<(String, i64, String)> = conn
        .prepare("SELECT kind,node,exec_id FROM flow_events ORDER BY seq")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        history
            .iter()
            .map(|(kind, node, _)| (kind.as_str(), *node))
            .collect::<Vec<_>>(),
        vec![
            ("operation_started", 0),
            ("operation_completed", 0),
            ("operation_started", 1),
            ("operation_completed", 1)
        ]
    );
    assert!(history.iter().all(|(_, _, exec)| exec == &history[0].2));
    assert_eq!(
        conn.query_row("SELECT state FROM flow_sessions", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "completed"
    );
}

#[test]
fn mechanical_failure_retains_earlier_success_in_the_same_exec() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    write_flow(
        repo.path(),
        "mechanical-failure",
        "- op: rebase --plan\n- op: __telemetry-scorecard\n",
    );
    let output = run_lf(
        repo.path(),
        home.path(),
        &["flow", "mechanical-failure", "-b", "--no-loopflow"],
        None,
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("telemetry scorecard generator not found"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let outcomes: Vec<(String, String)> = conn
        .prepare(
            "SELECT outcome,exec_id FROM flow_events WHERE kind='operation_completed' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        outcomes
            .iter()
            .map(|(outcome, _)| outcome.as_str())
            .collect::<Vec<_>>(),
        vec!["completed", "failed"]
    );
    assert_eq!(outcomes[0].1, outcomes[1].1);
    assert_eq!(
        conn.query_row("SELECT outcome FROM execs", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "failed"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
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
    lf_command(repo, home, args, path).output().unwrap()
}

fn lf_command(repo: &Path, home: &Path, args: &[&str], path: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_") {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(repo)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("NO_COLOR", "1");
    if let Some(path) = path {
        command.env("PATH", path);
    }
    command
}

fn publish_stack_fixture_pr(
    runtime: &tokio::runtime::Runtime,
    store: &loopflow::store::Store,
    task: &loopflow::work::task::TaskId,
) -> loopflow::work::task::TaskPr {
    let mut pr = runtime
        .block_on(store.active_task_pr(task))
        .unwrap()
        .unwrap();
    pr.publication = Some(loopflow::work::task::PrPublication {
        requested_at: pr.created_at,
        presentation: None,
        github: Some(loopflow::work::task::GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime.block_on(store.update_task_pr(&pr)).unwrap();
    pr
}

#[test]
fn task_checkout_selects_parent_without_rewriting_work_or_publication() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let child =
        support::register_unrun_task(home.path(), repo.path(), "child-task", &repo.head_sha());
    let parent_path = repo.create_named_worktree("parent-task");
    let parent = support::register_sibling_task(&child, "INF-124", "parent-task", &parent_path);
    repo.create_branch("child-task");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let task_before = runtime
        .block_on(child.store.get_task(&child.task.id))
        .unwrap()
        .unwrap();
    let before = publish_stack_fixture_pr(&runtime, &child.store, &child.task.id);
    let parent_pr = publish_stack_fixture_pr(&runtime, &child.store, &parent.id);
    repo.create_file("committed.txt", "child-authored commit");
    repo.stage_all();
    repo.commit("Child work before selecting its parent");
    repo.create_file("staged.txt", "staged bytes");
    repo.stage_all();
    repo.create_file("committed.txt", "unstaged bytes");
    repo.create_file("untracked.txt", "untracked bytes");
    let head = repo.head_sha();
    let index = Command::new("git")
        .args(["diff", "--cached", "--binary"])
        .current_dir(repo.path())
        .output()
        .unwrap()
        .stdout;
    let events = runtime
        .block_on(child.store.task_events_after(&child.task.id, 0))
        .unwrap();
    for _ in 0..2 {
        let output = lf_command(
            repo.path(),
            home.path(),
            &[
                "task",
                "checkout",
                "INF-123",
                "--stack-on",
                "INF-124",
                "--json",
            ],
            None,
        )
        .env_remove("LF_CONTROL_DB_PATH")
        .env("LF_DB_PATH", home.path().join("loopflow.db"))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let returned: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(returned.is_object());
        let after = runtime
            .block_on(child.store.active_task_pr(&child.task.id))
            .unwrap()
            .unwrap();
        let mut expected = before.clone();
        expected.parent_pr_id = Some(parent_pr.id.clone());
        expected.updated_at = after.updated_at;
        assert_eq!(after, expected);
        assert_eq!(
            runtime
                .block_on(child.store.get_task(&child.task.id))
                .unwrap()
                .unwrap(),
            task_before
        );
        assert_eq!(
            runtime
                .block_on(child.store.task_events_after(&child.task.id, 0))
                .unwrap(),
            events
        );
    }
    let selected = runtime
        .block_on(child.store.active_task_pr(&child.task.id))
        .unwrap();
    let failed = lf_command(
        repo.path(),
        home.path(),
        &[
            "task",
            "prepare",
            "INF-123",
            "--stack-on",
            "INF-404",
            "--json",
        ],
        None,
    )
    .env("LF_DB_PATH", home.path().join("loopflow.db"))
    .output()
    .unwrap();
    assert!(!failed.status.success());
    assert_eq!(
        runtime
            .block_on(child.store.active_task_pr(&child.task.id))
            .unwrap(),
        selected
    );
    // A successful publication observed from an older snapshot must survive,
    // while the dedicated parent writer retains the newer dependency.
    let mut publication = before.clone();
    publication
        .publication
        .as_mut()
        .unwrap()
        .github
        .as_mut()
        .unwrap()
        .head_sha = Some("published-after-selection".into());
    publication.linear_attachment_id = Some("linked-after-selection".into());
    runtime
        .block_on(child.store.update_task_pr(&publication))
        .unwrap();
    let recorded = runtime
        .block_on(child.store.get_task_pr(&before.id))
        .unwrap()
        .unwrap();
    assert_eq!(
        recorded.parent_pr_id,
        selected.as_ref().unwrap().parent_pr_id
    );
    assert_eq!(recorded.publication, publication.publication);
    assert_eq!(
        recorded.linear_attachment_id,
        publication.linear_attachment_id
    );
    assert_eq!(repo.head_sha(), head);
    assert_eq!(
        fs::read_to_string(repo.path().join("committed.txt")).unwrap(),
        "unstaged bytes"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
        "staged bytes"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("untracked.txt")).unwrap(),
        "untracked bytes"
    );
    assert_eq!(
        Command::new("git")
            .args(["diff", "--cached", "--binary"])
            .current_dir(repo.path())
            .output()
            .unwrap()
            .stdout,
        index
    );
    assert_eq!(
        runtime
            .block_on(child.store.get_task_pr(&parent_pr.id))
            .unwrap()
            .unwrap(),
        parent_pr
    );
}

#[test]
fn checkout_task_identity_ignores_main_and_parent_upstreams() {
    for upstream in ["main", "parent-task"] {
        let repo = loopflow_test_support::TestRepo::new();
        let home = TempDir::new().unwrap();
        let child =
            support::register_unrun_task(home.path(), repo.path(), "child-task", &repo.head_sha());
        let parent_path = repo.create_named_worktree("parent-task");
        let parent = support::register_sibling_task(&child, "INF-124", "parent-task", &parent_path);
        // Both tracking configurations are real Git refs, with no network.
        run_git(repo.path(), &["push", "origin", "parent-task"]);
        repo.create_branch("child-task");
        run_git(
            repo.path(),
            &["branch", "--set-upstream-to", &format!("origin/{upstream}")],
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        if upstream == "parent-task" {
            let parent_pr = publish_stack_fixture_pr(&runtime, &child.store, &parent.id);
            runtime
                .block_on(child.store.stack_task_pr(&child.pr, &parent_pr.id))
                .unwrap();
        }
        write_skill(repo.path(), "identity-proof", "Prove checkout identity.");
        write_flow(repo.path(), "identity-proof", "- step:\n    id: work\n    name: identity-proof\n- step:\n    id: decide\n    name: identity-proof\n    repeat:\n      from: work\n");
        let position = runtime
            .block_on(child.store.start_task_flow(
                &child.task.id,
                FlowSession {
                    task_id: Some(child.task.id.clone()),
                    parent_id: None,
                    wave_id: Some(child.task.wave_id.clone()),
                    cwd: child.task.worktree.clone(),
                    message: None,
                    model: None,
                    current_attempt: None,
                    finished: false,
                    invocation: QueuedInvocation::load(repo.path(), "identity-proof").unwrap(),
                    pending_session_id: None,
                    ready_summary: None,
                    cursor: loopflow::engine::ExecutionCursor {
                        index: 1,
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
        let parent_before = runtime
            .block_on(child.store.get_task(&parent.id))
            .unwrap()
            .unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: std::process::id(),
            started_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        };
        let claim = match runtime
            .block_on(child.store.claim_task_worker(
                &child.task.id,
                &position.invocation.id,
                position.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            ))
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            other => panic!("unexpected claim: {other:?}"),
        };
        let claimed = runtime
            .block_on(child.store.task_flow(&child.task.id))
            .unwrap()
            .unwrap();
        let reviewer = claimed.current_attempt.as_ref().unwrap().run_id.clone();
        runtime
            .block_on(child.store.publish_attempt(
                claimed.id(),
                claimed.version,
                claimed.current_attempt.as_ref().unwrap().captured,
                Some(&claim),
                "codex",
                None,
            ))
            .unwrap();
        // Exercise shared native-turn admission against a scripted provider. The
        // checkout decision must carry the caller installed in that exact turn.
        use loopflow::engine::agent::AgentConfig;
        use loopflow::harness::{codex::CodexHarness, ApprovalPolicy, Harness};
        let provider = codex_app_server_script("done", "")
            .replace("read -r thread_start", "read -r thread_start\nprintf '%s\\n' \"$thread_start\" > \"$LF_CONTROL_HOME/thread-request\"")
            .replace("printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"item/agentMessage/delta\"", "read -r release\nprintf '%s\\n' '{\"jsonrpc\":\"2.0\",\"method\":\"item/agentMessage/delta\"");
        let lf = format!("#!/bin/sh\nexec '{}' \"$@\"\n", env!("CARGO_BIN_EXE_lf"));
        let _env =
            support::EnvGuard::with_lf_home(&[("codex", &provider), ("lf", &lf)], home.path());
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        db.execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,?2,?3)",
            rusqlite::params![owner.exec_id, owner.trace_id, owner.started_at],
        )
        .unwrap();
        let store =
            loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
        let session = store.session_for_artifact(&reviewer).unwrap().unwrap();
        let driver = store
            .claim_session_driver(&session.id, None, &owner.exec_id, true)
            .unwrap();
        let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut harness = CodexHarness::new(events, ApprovalPolicy::AutoApprove);
        runtime.block_on(async {
            harness
                .start(&AgentConfig {
                    agent: Some("codex".into()),
                    cwd: Some(repo.path().into()),
                    env: std::collections::BTreeMap::from([(
                        "LF_AGENT_CALLER".into(),
                        serde_json::to_string(&driver.caller(session.id.clone())).unwrap(),
                    )]),
                    session_driver: Some((session.id.clone(), driver)),
                    flow_selection: Some(loopflow::durable::FlowTurnSelection {
                        output: None,
                        flow_id: claimed.id().into(),
                        version: claimed.version,
                        claim: Some(claim.clone()),
                        session_id: session.id,
                        after: 0,
                        caller_token: None,
                    }),
                    ..Default::default()
                })
                .await
                .unwrap();
            harness
                .send_input("Prove checkout identity.")
                .await
                .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                while let Some(event) = received.recv().await {
                    if matches!(
                        event,
                        loopflow::chat::types::ConversationEvent::TurnStarted { .. }
                    ) {
                        return;
                    }
                }
                panic!("fixture provider ended before its native start");
            })
            .await
            .unwrap();
        });
        let request: serde_json::Value =
            serde_json::from_slice(&fs::read(home.path().join("thread-request")).unwrap()).unwrap();
        let caller = request["params"]["config"]["shell_environment_policy.set"]["LF_AGENT_CALLER"]
            .as_str()
            .unwrap();
        let decision = lf_command(
            repo.path(),
            home.path(),
            &["task", "status", "--json"],
            None,
        )
        .env("LF_AGENT_CALLER", caller)
        .env("LF_RUN_ID", reviewer.as_str())
        .env(
            "LF_FLOW_STEP",
            serde_json::json!({"invocation": claimed.id(), "version": claimed.version}).to_string(),
        )
        .output()
        .unwrap();
        runtime.block_on(harness.stop()).unwrap();
        assert!(
            decision.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&decision.stderr)
        );
        let position = runtime
            .block_on(child.store.task_flow(&child.task.id))
            .unwrap()
            .unwrap();
        assert!(position.cursor.progress.verdict.is_none());
        let resolved: serde_json::Value = serde_json::from_slice(&decision.stdout).unwrap();
        assert_eq!(resolved["task_id"], child.task.id.as_str());
        assert!(runtime
            .block_on(child.store.task_flow(&parent.id))
            .unwrap()
            .is_none());

        // A subdirectory still resolves the registered checkout.
        let subdir = repo.path().join("nested");
        fs::create_dir(&subdir).unwrap();
        let status = run_lf(&subdir, home.path(), &["task", "status", "--json"], None);
        assert!(
            status.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(status["task_id"], child.task.id.as_str(), "{status}");

        let bin = TempDir::new().unwrap();
        let launched = bin.path().join("launched");
        write_executable(&bin.path().join("codex"), &format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\nprintf '%s' '{{\"schema_version\":1,\"provider_session_id\":\"ses-'\"$LF_RUN_ID\"'\",\"account_id\":null}}' > \"$LF_RUN_DIR/provider-session.json\"\necho \"$LF_RUN_ID\" > '{}'\n", launched.display(),
        ));
        let path = format!(
            "{}:{}",
            bin.path().display(),
            std::env::var("PATH").unwrap()
        );
        let launch = run_lf(
            repo.path(),
            home.path(),
            &["--tui", "identity-proof", "--no-loopflow"],
            Some(&path),
        );
        assert!(
            launch.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&launch.stderr)
        );
        let run_id = fs::read_to_string(launched).unwrap();
        let sessions = run_lf(
            repo.path(),
            home.path(),
            &["session", "list", "--all", "--json"],
            None,
        );
        assert!(
            sessions.status.success(),
            "{}",
            String::from_utf8_lossy(&sessions.stderr)
        );
        let sessions: serde_json::Value = serde_json::from_slice(&sessions.stdout).unwrap();
        let session = sessions
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["run_id"] == run_id.trim())
            .unwrap();
        assert_eq!(
            session["work"],
            serde_json::json!({"kind": "task", "id": child.task.id})
        );
        assert_eq!(
            runtime
                .block_on(child.store.get_task(&parent.id))
                .unwrap()
                .unwrap(),
            parent_before
        );
    }
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
    // Started is `tasks.started_at`, set by the first Run that names the Task.
    let started_at = || -> Option<i64> {
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT started_at FROM tasks WHERE id=?1",
                [task.task.id.as_str()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let starts = || started_at().iter().count();
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
    // A Flow about the Task names it on its review Run, and the first Run
    // that names a Task starts it, opened or not.
    assert_eq!(starts(), 1, "a review Run naming the Task starts it");
    assert!(
        started(),
        "a reserved Run naming the Task is start evidence"
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
    let mut first = None;
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
        assert_eq!(starts(), 1, "independent execution starts its Task");
        assert!(started(), "a launched Run is durable start evidence");
        assert_eq!(
            *first.get_or_insert(started_at()),
            started_at(),
            "a later Run leaves Started alone"
        );
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[test]
fn task_run_history_reads_only_that_tasks_runs_without_starting_it() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task =
        support::register_unrun_task(home.path(), repo.path(), "task-history", &repo.head_sha());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let events = || {
        runtime
            .block_on(task.store.task_events_after(&task.task.id, 0))
            .unwrap()
            .len()
    };
    let read = |selector: &str| -> Vec<serde_json::Value> {
        let output = run_lf(
            repo.path(),
            home.path(),
            &["runs", "--task", selector, "--json"],
            None,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };

    // An unstarted Task: an empty list, and asking neither prepares nor starts it.
    let before = events();
    assert!(read("INF-123").is_empty());
    assert_eq!(events(), before, "reading Run history writes no Task event");
    assert!(!runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());

    write_skill(repo.path(), "history-work", "Do proof-owned work.");
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
    // The same checkout runs one Task-bound and one unattributed Run.
    for args in [
        &["--task", "INF-123", "history-work", "-b", "--no-loopflow"][..],
        &["history-work", "-b", "--no-loopflow"][..],
    ] {
        let launched = run_lf(repo.path(), home.path(), args, Some(&path));
        assert!(
            launched.status.success(),
            "{}",
            String::from_utf8_lossy(&launched.stderr)
        );
    }

    let runs = read("INF-123");
    assert_eq!(
        runs.len(),
        1,
        "only the exact Task's Run, not its checkout: {runs:?}"
    );
    assert_eq!(runs[0]["harness"], "codex");
    assert_eq!(runs[0]["skill"], "history-work");
    assert!(runs[0]["id"].as_str().unwrap().starts_with("run_"));
    assert!(runs[0]["started"].as_i64().is_some());
    let after_launch = events();
    assert!(read("INF-999").is_empty(), "another Task sees none of them");
    assert_eq!(
        events(),
        after_launch,
        "reads leave the Task's events alone"
    );
}

#[test]
fn lf_launches_inside_a_task_checkout_bind_to_that_task() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task =
        support::register_unrun_task(home.path(), repo.path(), "task-binding", &repo.head_sha());
    let sibling_worktree = repo.create_named_worktree("task-sibling");
    let sibling =
        support::register_sibling_task(&task, "INF-124", "task-sibling", &sibling_worktree);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Started is `tasks.started_at`, set by the first Run that names the Task.
    let starts = |id: &loopflow::work::task::TaskId| {
        rusqlite::Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .query_row(
                "SELECT count(*) FROM tasks WHERE id=?1 AND started_at IS NOT NULL",
                [id.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    let events = || {
        runtime
            .block_on(task.store.task_events_after(&task.task.id, 0))
            .unwrap()
            .len()
    };
    let json = |args: &[&str]| -> serde_json::Value {
        let output = run_lf(repo.path(), home.path(), args, None);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    };
    let session = |run_id: &str| -> serde_json::Value {
        json(&["session", "list", "--all", "--json"])
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["run_id"] == run_id)
            .cloned()
            .unwrap_or_else(|| panic!("Session {run_id} is not listed"))
    };
    let task_runs = |identifier: &str| -> Vec<String> {
        json(&["runs", "--task", identifier, "--json"])
            .as_array()
            .unwrap()
            .iter()
            .map(|run| run["id"].as_str().unwrap().to_string())
            .collect()
    };

    write_skill(repo.path(), "binding-work", "Do proof-owned work.");
    // An explicit `--task` launch runs in that Task's worktree, which has its
    // own uncommitted catalog.
    write_skill(&sibling_worktree, "binding-work", "Do proof-owned work.");
    // A codex TUI stand-in. It records its provider Session the way the real
    // client's session-start hook does and notes which Run launched it.
    let bin = TempDir::new().unwrap();
    let launched = bin.path().join("launched");
    write_executable(
        &bin.path().join("codex"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\n\
             printf '%s' '{{\"schema_version\":1,\"provider_session_id\":\"ses-'\"$LF_RUN_ID\"'\",\"account_id\":null}}' \
             > \"$LF_RUN_DIR/provider-session.json\"\necho \"$LF_RUN_ID\" >> '{}'\n",
            launched.display()
        ),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let launch = |args: &[&str]| -> String {
        let before = std::fs::read_to_string(&launched).unwrap_or_default();
        let output = run_lf(repo.path(), home.path(), args, Some(&path));
        assert!(
            output.status.success(),
            "lf {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let after = std::fs::read_to_string(&launched).unwrap();
        after[before.len()..].trim().to_string()
    };

    assert_eq!(starts(&task.task.id), 0);

    // In the Task's checkout, a plain launch binds to that Task.
    repo.create_branch("task-binding");
    let bound = launch(&["--tui", "binding-work", "--no-loopflow"]);
    let listed = session(&bound);
    assert_eq!(
        listed["work"],
        serde_json::json!({"kind": "task", "id": task.task.id}),
        "{listed}"
    );
    assert_eq!(listed["wave_id"], serde_json::json!(task.task.wave_id));
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);
    assert_eq!(
        starts(&task.task.id),
        1,
        "a launch bound from its checkout is work beginning"
    );
    assert!(runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());

    // A branch no Task owns stays unbound; nothing is inferred from the path.
    repo.create_branch("unregistered");
    let unbound = launch(&["--tui", "binding-work", "--no-loopflow"]);
    assert_eq!(session(&unbound)["work"], serde_json::Value::Null);
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);

    // Explicit selection wins over the checkout.
    repo.checkout("task-binding");
    let explicit = launch(&[
        "--task",
        "INF-124",
        "--tui",
        "binding-work",
        "--no-loopflow",
    ]);
    assert_eq!(
        session(&explicit)["work"],
        serde_json::json!({"kind": "task", "id": sibling.id})
    );
    assert_eq!(task_runs("INF-124"), vec![explicit]);
    assert_eq!(task_runs("INF-123"), vec![bound.clone()]);
    assert_eq!(starts(&task.task.id), 1);
    assert_eq!(starts(&sibling.id), 1);

    // Observation stays observation.
    let settled = events();
    let _ = json(&["session", "list", "--all", "--json"]);
    let _ = json(&["runs", "--task", "INF-123", "--json"]);
    let _ = json(&["runs", "--active", "--task", "INF-123", "--json"]);
    assert_eq!(events(), settled, "reads write no Task event");

    // Once the branch's PR has landed with nothing after it, the checkout no
    // longer tracks the Task's work: a plain launch still works, unbound.
    let mut landed = task.pr.clone();
    landed.publication = Some(loopflow::work::task::PrPublication {
        requested_at: time::OffsetDateTime::now_utc(),
        presentation: None,
        github: Some(loopflow::work::task::GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: None,
        }),
        merge: None,
    });
    landed.merge_commit = Some(repo.head_sha());
    runtime
        .block_on(task.store.update_task_pr(&landed))
        .unwrap();
    let after_landing = launch(&["--tui", "binding-work", "--no-loopflow"]);
    assert_eq!(session(&after_landing)["work"], serde_json::Value::Null);
    assert_eq!(task_runs("INF-123"), vec![bound]);
}

#[test]
fn historical_start_evidence_still_prevents_backlog_retirement() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task = support::register_unrun_task(
        home.path(),
        repo.path(),
        "historical-start",
        &repo.head_sha(),
    );
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    db.execute("INSERT INTO task_events(task_id,kind_json,created_at) VALUES(?1,'{\"kind\":\"started\"}',1)", [task.task.id.as_str()]).unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert!(!runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());
    assert!(
        runtime
            .block_on(task.store.chapter_task_evidence(&task.task.id))
            .unwrap()
            .begun
    );
    assert!(!runtime
        .block_on(task.store.retire_chapter_backlog(&task.task.id))
        .unwrap());
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn task_operation_starts_with_durable_history_after_claim_only_failure() {
    use loopflow::durable::{FlowSession, TaskWorkerClaimOutcome, TaskWorkerOwner};
    use loopflow::engine::invocation::QueuedInvocation;
    let repo = loopflow_test_support::TestRepo::new();
    repo.create_branch("task-claim");
    let home = TempDir::new().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let task =
        support::register_unrun_task(home.path(), repo.path(), "task-claim", &repo.head_sha());
    write_flow(repo.path(), "claim-proof", "- op: rebase --plan\n");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let flow = runtime
        .block_on(task.store.start_task_flow(
            &task.task.id,
            FlowSession {
                parent_id: None,
                invocation: QueuedInvocation::load(repo.path(), "claim-proof").unwrap(),
                cursor: Default::default(),
                version: 0,
                task_id: Some(task.task.id.clone()),
                wave_id: Some(task.task.wave_id.clone()),
                cwd: repo.path().to_owned(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            },
        ))
        .unwrap();
    let owner = TaskWorkerOwner {
        trace_id: loopflow::id::TraceId::new(),
        exec_id: loopflow::id::ExecId::new(),
        pid: std::process::id(),
        started_at: 1,
    };
    let TaskWorkerClaimOutcome::Claimed(claim) = runtime
        .block_on(task.store.claim_task_worker(
            &task.task.id,
            flow.id(),
            flow.version,
            &owner,
            time::OffsetDateTime::now_utc(),
        ))
        .unwrap()
    else {
        panic!("claim")
    };
    let reserved = runtime
        .block_on(task.store.flow(flow.id()))
        .unwrap()
        .unwrap();
    assert!(reserved.current_attempt.is_none());
    assert!(!runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());
    let released = runtime
        .block_on(
            task.store
                .release_flow(flow.id(), reserved.version, Some(&claim)),
        )
        .unwrap();
    assert!(!runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());
    let TaskWorkerClaimOutcome::Claimed(claim) = runtime
        .block_on(task.store.claim_task_worker(
            &task.task.id,
            flow.id(),
            released.version,
            &owner,
            time::OffsetDateTime::now_utc(),
        ))
        .unwrap()
    else {
        panic!("replacement claim");
    };
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM task_events WHERE json_extract(kind_json,'$.kind')='started'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let read = run_lf(repo.path(), home.path(), &["roadmap", "--json"], None);
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(
        roadmap["waves"][0]["tasks"]["items"][0]["runtime"]["started"], false,
        "{roadmap}"
    );
    // The worker resumes the captured graph after its template has gone.
    fs::remove_file(repo.path().join(".lf/flows/claim-proof.yaml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["task", "__worker", task.task.id.as_str()])
        .current_dir(repo.path())
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_DB_PATH", home.path().join("loopflow.db"))
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env(
            loopflow::durable::TASK_WORKER_CLAIM_ENV,
            serde_json::to_string(&claim).unwrap(),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let started: i64 = db
        .query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM flow_events WHERE flow_id=?1 AND kind='operation_started'",
            [flow.id()],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    let read = run_lf(
        repo.path(),
        home.path(),
        &["runs", "--task", "INF-123", "--json"],
        None,
    );
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let runs: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(runs, serde_json::json!([]));
    assert_eq!(
        db.query_row(
            "SELECT started_at FROM tasks WHERE id=?1",
            [task.task.id.as_str()],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        started
    );
    assert!(
        runtime
            .block_on(task.store.flow(flow.id()))
            .unwrap()
            .unwrap()
            .finished
    );
    assert!(runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .is_none());
    assert!(runtime
        .block_on(task.store.task_started(&task.task.id))
        .unwrap());
    // Invocation-addressed resume retains the Task's recovery policy, even
    // with --retry: a restart-only failure must not reach a provider.
    let retry = runtime
        .block_on(task.store.start_task_flow(
            &task.task.id,
            FlowSession {
                parent_id: None,
                invocation:
                    QueuedInvocation::new("restart-proof", flow.invocation.steps.clone()).unwrap(),
                ..flow.clone()
            },
        ))
        .unwrap();
    let blocked = runtime
        .block_on(task.store.fail_flow(
            retry.id(),
            retry.version,
            None,
            &loopflow::durable::TaskFlowBlocker {
                captured: None,
                reason: "explicit restart required".into(),
                restart_required: true,
                observed_at: time::OffsetDateTime::now_utc(),
            },
        ))
        .unwrap();
    let output = run_lf(
        repo.path(),
        home.path(),
        &["flow", "resume", retry.id(), "--retry"],
        None,
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("lf task restart INF-123"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        runtime
            .block_on(task.store.flow(retry.id()))
            .unwrap()
            .unwrap(),
        blocked
    );
}

#[test]
fn bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone() {
    use loopflow::durable::FlowSession;
    use loopflow::engine::invocation::QueuedInvocation;
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
        .block_on(task.store.start_task_flow(
            &task.task.id,
            FlowSession {
                parent_id: None,
                invocation: QueuedInvocation::load(repo.path(), "code").unwrap(),
                cursor: loopflow::engine::ExecutionCursor {
                    index: 1,
                    iteration: 4,
                    ..Default::default()
                },
                version: 0,
                task_id: Some(task.task.id.clone()),
                wave_id: Some(task.task.wave_id.clone()),
                cwd: task.task.worktree.clone(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
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
                .block_on(task.store.task_flow(&task.task.id))
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
        .find(|s| s["kind"] == "flow")
        .unwrap();
    // A Flow about the Task names it on its review too, without becoming the
    // Task's Flow.
    assert_eq!(
        session["work"],
        serde_json::json!({"kind": "task", "id": task.task.id})
    );
    assert_eq!(session["flow_membership"]["flow"], "review-contribution");
    assert_eq!(session["flow_membership"]["occurrence"], "current");
    assert_eq!(session["flow_membership"]["node"], 0);
    let run_id = session["run_id"].as_str().unwrap();
    let listed = run_lf(
        repo.path(),
        home.path(),
        &["runs", "--task", "INF-123", "--json"],
        None,
    );
    assert!(listed.status.success());
    let listed: Vec<serde_json::Value> = serde_json::from_slice(&listed.stdout).unwrap();
    assert!(listed.iter().any(|run| run["id"] == run_id), "{listed:?}");
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
            .block_on(task.store.task_flow(&task.task.id))
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
- realign
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
fn scheduled_release_flow_propagates_the_operation_failure() {
    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    write_executable(&bin.join("gh"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &bin.join("release-publisher"),
        "#!/bin/sh\necho 'fixture publisher unavailable' >&2\nexit 27\n",
    );
    write_flow(
        repo.path(),
        "release-run",
        include_str!("../../../.lf/flows/release-run.yaml"),
    );
    fs::write(
        repo.path().join(".lf/config.yaml"),
        "release:\n  targets:\n    default:\n      publisher: [release-publisher]\n",
    )
    .unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());

    let output = run_lf(
        repo.path(),
        home.path(),
        &["--batch", "flow", "release-run"],
        Some(&path),
    );

    assert!(
        !output.status.success(),
        "a failed release must fail its scheduled target"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fixture publisher unavailable"), "{stderr}");
    assert!(!stderr.contains("launching agent"), "{stderr}");
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
- realign
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
    write_skill(repo, "review", "Review the supplied evidence.");
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

fn roadmap_flow(repo: &Path, home: &Path) -> serde_json::Value {
    let read = run_lf(repo, home, &["roadmap", "--json"], None);
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    roadmap["waves"][0]["tasks"]["items"][0]["flow"].clone()
}

fn unavailable(flow: &serde_json::Value, kind: &str) -> Option<String> {
    flow["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|control| control["kind"] == kind)
        .unwrap_or_else(|| panic!("{kind} control is projected"))["unavailable"]
        .as_str()
        .map(str::to_string)
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn task_flow_read_pins_topology_counts_both_returns_and_rejects_a_bad_restart() {
    use loopflow::durable::{FlowSession, TaskFlowBlocker};
    use loopflow::engine::invocation::QueuedInvocation;

    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task =
        support::register_unrun_task(home.path(), repo.path(), "task-flow-read", &repo.head_sha());
    run_git(repo.path(), &["branch", "task-flow-read"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for skill in [
        "design-proof",
        "implement-proof",
        "decide-proof",
        "demo-proof",
    ] {
        write_skill(repo.path(), skill, "Fixture step.");
    }
    let two_loops = "- step:\n    id: design\n    name: design-proof\n- step:\n    id: implement\n    name: implement-proof\n- step:\n    id: decide\n    name: decide-proof\n    repeat:\n      from: implement\n- step:\n    id: demo\n    name: demo-proof\n    human: true\n- step:\n    id: decide_delivery\n    name: decide-proof\n    repeat:\n      from: implement\n- op: pr land -c\n";
    write_flow(repo.path(), "two-loops", two_loops);

    // Before any Flow: the recommendation, Start, and no invented history.
    let flow = roadmap_flow(repo.path(), home.path());
    assert_eq!(flow["recommended"], "feature");
    assert_eq!(flow["record"]["kind"], "none");
    assert_eq!(unavailable(&flow, "start"), None);
    assert!(unavailable(&flow, "resume").is_some());

    // The catalogue previews the authored topology through the shared loader.
    let catalog = run_lf(repo.path(), home.path(), &["flow", "list", "--json"], None);
    assert!(
        catalog.status.success(),
        "{}",
        String::from_utf8_lossy(&catalog.stderr)
    );
    let catalog: Vec<serde_json::Value> = serde_json::from_slice(&catalog.stdout).unwrap();
    let preview = catalog
        .iter()
        .find(|entry| entry["name"] == "two-loops")
        .unwrap();
    let returns: Vec<_> = preview["graph"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["returns_to"].as_u64())
        .collect();
    assert_eq!(returns, [None, None, Some(1), None, Some(1), None]);
    assert!(catalog
        .iter()
        .any(|entry| entry["name"] == "feature" && entry["graph"].is_object()));

    // Pin the definition at iteration three with independent return counts.
    let pinned = runtime
        .block_on(task.store.start_task_flow(
            &task.task.id,
            FlowSession {
                parent_id: None,
                invocation: QueuedInvocation::load(repo.path(), "two-loops").unwrap(),
                cursor: loopflow::engine::ExecutionCursor {
                    index: 1,
                    iteration: 3,
                    progress: loopflow::engine::transitions::FlowProgress {
                        repeats: std::collections::BTreeMap::from([
                            ("decide".into(), 2),
                            ("decide_delivery".into(), 1),
                        ]),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                version: 0,
                task_id: Some(task.task.id.clone()),
                wave_id: Some(task.task.wave_id.clone()),
                cwd: task.task.worktree.clone(),
                message: None,
                model: None,
                current_attempt: None,
                pending_session_id: None,
                ready_summary: None,
                worker_generation: 0,
                claim: None,
                failure: None,
                finished: false,
                updated_at: time::OffsetDateTime::now_utc(),
            },
        ))
        .unwrap();
    // Editing the source changes new previews, never the pinned drawing.
    write_flow(
        repo.path(),
        "two-loops",
        "- step:\n    id: implement\n    name: implement-proof\n",
    );
    let flow = roadmap_flow(repo.path(), home.path());
    let record = &flow["record"];
    assert_eq!(record["kind"], "pinned", "{flow}");
    let labels: Vec<_> = record["graph"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["label"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        labels,
        [
            "design-proof",
            "implement-proof",
            "decide-proof",
            "demo-proof",
            "decide-proof",
            "pr land -c"
        ]
    );
    assert_eq!(record["current"], 1);
    assert_eq!(record["completed"], serde_json::json!([0]));
    assert_eq!(record["iterations"], serde_json::json!([[2, 1]]));
    assert_eq!(
        record["returns"],
        serde_json::json!([
            {"decider": 2, "traversals": 2},
            {"decider": 4, "traversals": 1}
        ])
    );
    assert_eq!(record["execution"], "idle");
    assert!(unavailable(&flow, "start")
        .unwrap()
        .contains("already pinned"));

    // A replacement that cannot load is rejected before refresh, checkpoint,
    // or stop: the pinned position is byte-for-byte unchanged.
    let head = repo.head_sha();
    let rejected = run_lf(
        repo.path(),
        home.path(),
        &["task", "restart", "INF-123", "--flow", "missing-flow"],
        None,
    );
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("missing-flow"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    let after = runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(after, pinned);
    assert_eq!(repo.head_sha(), head, "no restart checkpoint was committed");

    // A durable restart-only blocker is red and cannot be resumed.
    runtime
        .block_on(task.store.fail_flow(
            &after.invocation.id,
            after.version,
            None,
            &TaskFlowBlocker {
                captured: None,
                reason: "Release target is unavailable".into(),
                restart_required: true,
                observed_at: time::OffsetDateTime::now_utc(),
            },
        ))
        .unwrap();
    let flow = roadmap_flow(repo.path(), home.path());
    assert_eq!(flow["record"]["execution"], "blocked");
    assert_eq!(flow["record"]["restart_required"], true);
    assert!(unavailable(&flow, "resume")
        .unwrap()
        .contains("Only Stop & restart"));
    let status = run_lf(
        repo.path(),
        home.path(),
        &["task", "status", "INF-123"],
        None,
    );
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status = String::from_utf8(status.stdout).unwrap();
    assert_eq!(status.lines().next(), Some("INF-123  blocked"));
    assert!(status.contains("Release target is unavailable"));
}
