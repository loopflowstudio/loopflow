#[path = "support/chapter.rs"]
mod chapter;
mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::durable::{FlowPosition, RunId, TaskWorkerClaimOutcome, TaskWorkerOwner};
use loopflow::engine::flow::{ConcreteStep, Skill, Step};
use loopflow::engine::invocation::QueuedInvocation;
use loopflow::engine::transitions::FlowDecision;
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
        let mut child_pr = child.pr.clone();
        if upstream == "parent-task" {
            child_pr.parent_pr_id = Some(
                runtime
                    .block_on(child.store.active_task_pr(&parent.id))
                    .unwrap()
                    .unwrap()
                    .id,
            );
            runtime
                .block_on(child.store.update_task_pr(&child_pr))
                .unwrap();
        }
        write_skill(repo.path(), "identity-proof", "Prove checkout identity.");
        write_flow(repo.path(), "identity-proof", "- step:\n    id: work\n    name: identity-proof\n- step:\n    id: decide\n    name: identity-proof\n    repeat:\n      from: work\n");
        let position = runtime
            .block_on(child.store.set_flow_position(
                &child.task.id,
                FlowPosition {
                    task_id: child.task.id.clone(),
                    invocation: QueuedInvocation::load(repo.path(), "identity-proof").unwrap(),
                    session_run_id: None,
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
        let reviewer = RunId::new();
        runtime
            .block_on(
                child
                    .store
                    .bind_task_worker_run(&child.task.id, &claim, &reviewer, &owner),
            )
            .unwrap();
        let decision = lf_command(
            repo.path(),
            home.path(),
            &["flow", "decide", "iterate", "Checkout proof"],
            None,
        )
        .env("LF_RUN_ID", reviewer.as_str())
        .output()
        .unwrap();
        assert!(
            decision.status.success(),
            "{upstream}: {}",
            String::from_utf8_lossy(&decision.stderr)
        );
        let position = runtime
            .block_on(child.store.flow_position(&child.task.id))
            .unwrap()
            .unwrap();
        assert_eq!(
            position.cursor.progress.verdict.unwrap().decision,
            FlowDecision::Iterate
        );
        assert!(runtime
            .block_on(child.store.flow_position(&parent.id))
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
            &["--tui", "skill", "identity-proof", "--no-loopflow"],
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
    assert!(
        matches!(&flow.items[0].target, loopflow::engine::target::Target::Skill(skill) if skill.name == "implement")
    );
    assert_eq!(
        flow.items[1],
        Step {
            target: loopflow::engine::target::Target::Skill(Skill {
                name: "review".to_string(),
                agent: None,
                default_agent: None,
                action_style: None,
                content: None,
            }),
            id: None,
            human: false,
            repeat: None,
        }
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
    let starts = |id: &loopflow::work::task::TaskId| {
        runtime
            .block_on(task.store.task_events_after(id, 0))
            .unwrap()
            .into_iter()
            .filter(|event| event.kind == loopflow::work::task::TaskEventKind::Started)
            .count()
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
fn bound_flows_keep_task_context_and_leave_managed_flow_and_shared_edits_alone() {
    use loopflow::durable::FlowPosition;
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
    // A real collision: bare and explicit run select the flow; typed skill
    // selects the single skill, including when Work-bound.
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
        vec!["--task", "INF-123", "contribution"],
        vec!["--task", "INF-123", "run", "contribution"],
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
    assert_eq!(runs.len(), 10);
    for run in runs {
        assert!(run["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|subject| subject["selector"] == format!("task:{}", task.task.plan.identifier)));
        assert_eq!(run["outcome"], "completed");
    }
    for invocation in [vec!["skill", "contribution"], vec!["design"]] {
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
    assert_eq!(session["flow_membership"]["occurrence"], "current");
    assert_eq!(session["flow_membership"]["node"], "0");
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
fn flow_names_load_into_targets() {
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
    assert!(matches!(
        &flow.items[0].target,
        loopflow::engine::target::Target::Flow(_)
    ));
    assert!(matches!(
        flow.items[1],
        Step {
            target: loopflow::engine::target::Target::Skill(_),
            ..
        }
    ));
}

#[test]
fn command_item_parses_and_expands() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_flow(
        repo,
        "ship-ish",
        r#"
- implement
- cmd: pr land
"#,
    );

    let flow = load_flow("ship-ish", repo).unwrap();
    assert_eq!(flow.items.len(), 2);
    match &flow.items[1] {
        Step {
            target: loopflow::engine::target::Target::Command(item),
            ..
        } => {
            assert_eq!(item.command, "pr");
            assert_eq!(item.args, vec!["land"]);
        }
        other => panic!("expected command item, got {other:?}"),
    }

    let expanded = expand_flow(&flow, repo).unwrap();
    assert!(matches!(&expanded[1], ConcreteStep::Command(_)));
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
    write_flow(repo, "parent", "- step: review\n- publish");

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

#[test]
fn adding_a_flow_changes_an_untyped_reference_but_not_an_explicit_skill() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    write_skill(repo, "custom-review", "Review the code.");
    write_skill(repo, "replacement", "Follow the new review workflow.");
    write_flow(repo, "parent", "- custom-review\n- step: custom-review");
    let initial = expand_named_flow(repo, "parent");
    assert_skill_name(&initial[0], "custom-review");
    write_flow(repo, "custom-review", "- step: replacement");
    let changed = expand_named_flow(repo, "parent");
    assert_skill_name(&changed[0], "replacement");
    assert_skill_name(&changed[1], "custom-review");
}

#[test]
fn builtin_deploy_uses_ops_land_item() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let items = expand_named_flow(repo, "deploy");
    assert!(!items.is_empty());
    assert!(matches!(&items[1], ConcreteStep::Command(_)));
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
fn task_flow_read_pins_topology_counts_both_returns_and_projects_a_blocker() {
    use loopflow::durable::{FlowPosition, TaskFlowBlocker};
    use loopflow::engine::invocation::QueuedInvocation;

    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let task =
        support::register_unrun_task(home.path(), repo.path(), "task-flow-read", &repo.head_sha());
    run_git(repo.path(), &["branch", "task-flow-read"]);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let project = runtime
        .block_on(task.store.get_project(&task.task.project_id))
        .unwrap()
        .unwrap();
    runtime
        .block_on(task.store.save_chapter(
            &chapter::current_chapter(
                &task.task.wave_id,
                "task-pr-tests",
                project.plan.id.as_str(),
            ),
            true,
        ))
        .unwrap();
    for skill in [
        "design-proof",
        "implement-proof",
        "decide-proof",
        "demo-proof",
    ] {
        write_skill(repo.path(), skill, "Fixture step.");
    }
    let two_loops = "- step:\n    id: design\n    name: design-proof\n- step:\n    id: implement\n    name: implement-proof\n- step:\n    id: decide\n    name: decide-proof\n    repeat:\n      from: implement\n- step:\n    id: demo\n    name: demo-proof\n    human: true\n- step:\n    id: decide_delivery\n    name: decide-proof\n    repeat:\n      from: implement\n- cmd: pr land -c\n";
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
        .map(|node| node["returns_to"].as_str().map(str::to_string))
        .collect();
    assert_eq!(
        returns,
        [None, None, Some("1".into()), None, Some("1".into()), None]
    );
    assert!(catalog
        .iter()
        .any(|entry| entry["name"] == "feature" && entry["graph"].is_object()));

    // Pin the definition at iteration three with independent return counts.
    let pinned = runtime
        .block_on(task.store.set_flow_position(
            &task.task.id,
            FlowPosition {
                task_id: task.task.id.clone(),
                invocation: QueuedInvocation::load(repo.path(), "two-loops").unwrap(),
                session_run_id: None,
                ready_summary: None,
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
                worker_generation: 0,
                claim: None,
                failure: None,
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
    assert_eq!(record["current"], "1");
    assert_eq!(record["completed"], serde_json::json!(["0"]));
    assert_eq!(record["iterations"], serde_json::json!([[2, 1]]));
    assert_eq!(
        record["returns"],
        serde_json::json!([
            {"decider": "2", "traversals": 2},
            {"decider": "4", "traversals": 1}
        ])
    );
    assert_eq!(record["execution"], "idle");
    assert!(unavailable(&flow, "start")
        .unwrap()
        .contains("already pinned"));

    // A durable restart-only blocker is red and cannot be resumed.
    runtime
        .block_on(task.store.set_flow_position(
            &task.task.id,
            FlowPosition {
                failure: Some(TaskFlowBlocker {
                    run_id: None,
                    reason: "Release target is unavailable".into(),
                    restart_required: true,
                    observed_at: time::OffsetDateTime::now_utc(),
                }),
                ..pinned
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

#[test]
fn mixed_provider_flow_keeps_launch_accounts_after_driver_exit() {
    use base64::Engine;
    use loopflow::store::{
        CredentialState, ProviderAccount, ProviderAccountId, RoutingState, StorageConfig,
    };
    use sha2::{Digest, Sha256};

    let repo = loopflow_test_support::TestRepo::new();
    let home = TempDir::new().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.path().join("loopflow.db")),
        ))
        .unwrap();
    for provider in ["claude", "codex"] {
        for label in ["chosen", "other"] {
            let id = format!("{provider}-{label}");
            let email = format!("{id}@example.com");
            let account_home = home.path().join("accounts").join(provider).join(&id);
            fs::create_dir_all(&account_home).unwrap();
            let credential = if provider == "claude" {
                serde_json::json!({"claudeAiOauth":{"accessToken":format!("fixture-{id}"),"expiresAt":4102444800000i64}}).to_string()
            } else {
                let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(serde_json::json!({"email":email,"sub":id}).to_string());
                serde_json::json!({"tokens":{"access_token":"fixture", "id_token":format!("h.{claims}.s")}}).to_string()
            };
            fs::write(
                account_home.join(if provider == "claude" {
                    ".credentials.json"
                } else {
                    "auth.json"
                }),
                &credential,
            )
            .unwrap();
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            runtime
                .block_on(
                    store.upsert_provider_account(&ProviderAccount {
                        provider: provider.into(),
                        account_id: ProviderAccountId::parse(&id).unwrap(),
                        home: Some(account_home),
                        login_email: Some(loopflow::profile::EmailAddress::parse(&email).unwrap()),
                        observed_email: Some(email),
                        observed_subject: Some(id),
                        observed_credential_digest: (provider == "claude")
                            .then(|| format!("{:x}", Sha256::digest(credential.as_bytes()))),
                        observed_plan: None,
                        credential_state: CredentialState::Connected,
                        // Explicit selection must work even when automatic routing prefers another login.
                        routing_state: if label == "chosen" {
                            RoutingState::ExplicitOnly
                        } else {
                            RoutingState::Automatic
                        },
                        plan: None,
                        paid_through: None,
                        utilization_percent: None,
                        cooldown_until: None,
                        cooldown_reason: None,
                        last_selected_at: None,
                        created_at: now,
                        updated_at: now,
                    }),
                )
                .unwrap();
        }
    }
    for (skill, provider) in [
        ("c1", "claude"),
        ("d1", "codex"),
        ("c2", "claude"),
        ("d2", "codex"),
        ("d-review", "codex"),
    ] {
        write_skill(
            repo.path(),
            skill,
            &format!("---\nagent: {provider}\n---\nRun {skill}."),
        );
    }
    write_flow(
        repo.path(),
        "pair",
        "- c1\n- d1\n- c2\n- d2\n- step:\n    id: review\n    name: d-review\n    human: true\n",
    );
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "mixed provider fixture"]);
    let bin = TempDir::new().unwrap();
    write_executable(
        &bin.path().join("claude"),
        r#"#!/bin/sh
case "$1" in --version) exit 0;; esac
printf 'claude:%s\n' "$CLAUDE_CONFIG_DIR" >> "$LF_HOME/selected"
if [ -f "$LF_HOME/first-claude" ] && [ ! -f "$LF_HOME/retry" ]; then exit 23; fi
touch "$LF_HOME/first-claude"
cat >/dev/null
echo done
"#,
    );
    write_executable(
        &bin.path().join("codex"),
        &codex_app_server_script(
            "done",
            r#"if [ "$1" = --version ]; then exit 0; fi
printf 'codex:%s\n' "$CODEX_HOME" >> "$LF_HOME/selected"
case "$*" in *app-server*) ;; *)
  printf '%s' '{"schema_version":1,"provider_session_id":"review-fixture","account_id":null}' > "$LF_RUN_DIR/provider-session.json"
  exit 0;; esac"#,
        ),
    );
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap()
    );
    let output = run_lf(
        repo.path(),
        home.path(),
        &[
            "--account",
            "claude=claude-chosen@",
            "--account",
            "codex=codex-chosen@",
            "-b",
            "flow",
            "pair",
        ],
        Some(&path),
    );
    assert!(
        !output.status.success(),
        "fixture pauses on second Claude step"
    );
    let flow_dir = fs::read_dir(home.path().join("flows"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(flow_dir.join("position.json")).unwrap()).unwrap();
    assert_eq!(
        saved["cursor"]["index"],
        2,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(home.path().join("retry"), "").unwrap();
    // A new CLI has no broker and no account flags. It must recover saved intent.
    let output = run_lf(
        repo.path(),
        home.path(),
        &[
            "-b",
            "flow",
            "resume",
            saved["id"].as_str().unwrap(),
            "--retry",
        ],
        Some(&path),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("waiting for human input"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sessions = run_lf(
        repo.path(),
        home.path(),
        &["session", "list", "--json"],
        Some(&path),
    );
    let sessions: serde_json::Value = serde_json::from_slice(&sessions.stdout).unwrap();
    let session = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["kind"] == "flow")
        .unwrap_or(&sessions[0]);
    let output = run_lf(
        repo.path(),
        home.path(),
        &["session", "open", session["id"].as_str().unwrap()],
        Some(&path),
    );
    // The TUI fixture exits immediately; it proves account delivery, not native resume.
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("before becoming resumable"));
    let selected = fs::read_to_string(home.path().join("selected")).unwrap();
    let expected = ["claude", "codex", "claude", "claude", "codex", "codex"].map(|provider| {
        format!(
            "{provider}:{}",
            home.path()
                .join("accounts")
                .join(provider)
                .join(format!("{provider}-chosen"))
                .display()
        )
    });
    assert_eq!(
        selected.lines().collect::<Vec<_>>(),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
}
