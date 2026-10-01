mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use loopflow::ops::task::task_status;
use loopflow::ops::task_actions::TaskAction;
use loopflow::work::task::{GithubPr, PrPublication, TaskEventKind};
use loopflow_test_support::TestRepo;
use sha2::{Digest, Sha256};
use support::{register_unrun_task, EnvGuard};

fn unbound_command(cli: &Path, repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(cli);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    command.current_dir(repo).args(args);
    command
}

#[test]
fn stacked_checkout_starts_with_one_scratch_deletion_commit() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    repo.create_branch("parent");
    repo.create_file("scratch/design.md", "parent design");
    repo.create_file("scratch/review/notes.md", "parent review");
    repo.stage_all();
    repo.commit("Parent notes");
    let parent_head = repo.head_sha();
    let parent = register_unrun_task(home.path(), repo.path(), "parent", &parent_head);
    let child =
        support::register_sibling_task(&parent, "INF-124", "child", &target.path().join("child"));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut parent_pr = parent.pr.clone();
    parent_pr.publication = Some(PrPublication {
        requested_at: parent_pr.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 41,
            url: "https://github.com/fixture/repo/pull/41".into(),
            head_sha: Some(parent_pr.base_commit.clone()),
        }),
        merge: None,
    });
    runtime
        .block_on(parent.store.update_task_pr(&parent_pr))
        .unwrap();
    let pr = runtime
        .block_on(parent.store.active_task_pr(&child.id))
        .unwrap()
        .unwrap();
    runtime
        .block_on(parent.store.stack_task_pr(&pr, &parent.pr.id))
        .unwrap();

    let checkout = || {
        loopflow::ops::task::task_checkout(
            repo.path(),
            "INF-124",
            loopflow::ops::task::TaskCheckoutOptions::default(),
        )
        .unwrap()
    };
    checkout();
    assert!(!child.worktree.join("scratch").exists());
    assert_eq!(
        loopflow::engine::git::rev_parse(&child.worktree, "HEAD^").unwrap(),
        parent_head
    );
    let subject = Command::new("git")
        .current_dir(&child.worktree)
        .args(["log", "-1", "--format=%s"])
        .output()
        .unwrap();
    assert!(subject.status.success());
    assert_eq!(
        String::from_utf8_lossy(&subject.stdout).trim(),
        "Clear inherited scratch"
    );
    let child_head = loopflow::engine::git::rev_parse(&child.worktree, "HEAD").unwrap();
    fs::create_dir(child.worktree.join("scratch")).unwrap();
    fs::write(child.worktree.join("scratch/design.md"), "child design").unwrap();
    checkout();
    assert_eq!(
        loopflow::engine::git::rev_parse(&child.worktree, "HEAD").unwrap(),
        child_head
    );
    assert_eq!(
        fs::read_to_string(child.worktree.join("scratch/design.md")).unwrap(),
        "child design"
    );
    assert_eq!(repo.head_sha(), parent_head);
    assert_eq!(
        fs::read_to_string(repo.path().join("scratch/design.md")).unwrap(),
        "parent design"
    );
    assert!(repo.path().join("scratch/review/notes.md").exists());
}

#[test]
fn checkout_restores_exact_task_history_from_a_dirty_checkout() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let mut fixture = register_unrun_task(
        home.path(),
        repo.path(),
        "test/checkout-recovery",
        &repo.head_sha(),
    );
    fixture.task.worktree = target.path().join("checkout");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime
        .block_on(fixture.store.update_task(&fixture.task))
        .unwrap();
    fixture.pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap();
    let invoking = repo.create_named_worktree("dirty-invoker");
    fs::write(repo.path().join("main-notes"), "keep main edits").unwrap();
    fs::write(invoking.join("caller-notes"), "keep caller edits").unwrap();
    let checkout = || {
        unbound_command(
            Path::new(env!("CARGO_BIN_EXE_lf")),
            &invoking,
            &["task", "checkout", "INF-123", "--json"],
        )
        .env("LF_HOME", home.path())
        .env("LF_DB_PATH", home.path().join("loopflow.db"))
        .output()
        .unwrap()
    };
    let first = checkout();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap(),
        fixture.pr.base_commit
    );
    fs::write(
        fixture.task.worktree.join("work.txt"),
        "committed Task work",
    )
    .unwrap();
    for args in [
        vec!["add", "work.txt"],
        vec!["commit", "-m", "Preserve Task work"],
    ] {
        assert!(Command::new("git")
            .current_dir(&fixture.task.worktree)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    let head = loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap();
    fs::remove_dir_all(&fixture.task.worktree).unwrap();
    let restored = checkout();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert_eq!(
        loopflow::engine::git::rev_parse(&fixture.task.worktree, "HEAD").unwrap(),
        head
    );
    assert_eq!(
        fs::read_to_string(fixture.task.worktree.join("work.txt")).unwrap(),
        "committed Task work"
    );
    let persisted = runtime
        .block_on(fixture.store.get_task(&fixture.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(persisted.worktree, fixture.task.worktree);
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap(),
        fixture.pr
    );
    let events = runtime
        .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
        .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.kind, TaskEventKind::PrStarted { .. }))
            .count(),
        1
    );
    fs::remove_dir_all(&fixture.task.worktree).unwrap();
    fs::create_dir(&fixture.task.worktree).unwrap();
    fs::write(fixture.task.worktree.join("notes"), "unregistered work").unwrap();
    let occupied = checkout();
    assert!(!occupied.status.success());
    assert!(String::from_utf8_lossy(&occupied.stderr).contains("occupied"));
    assert_eq!(
        fs::read_to_string(fixture.task.worktree.join("notes")).unwrap(),
        "unregistered work"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("main-notes")).unwrap(),
        "keep main edits"
    );
    assert_eq!(
        fs::read_to_string(invoking.join("caller-notes")).unwrap(),
        "keep caller edits"
    );
}

#[test]
fn task_live_unblock_status_and_desktop_share_exact_boundary_and_recovery() {
    use loopflow::durable::{FlowSession, TaskWorkerClaimOutcome, TaskWorkerOwner};
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_branch("jack/live-unblock");
    let task = register_unrun_task(
        home.path(),
        repo.path(),
        "jack/live-unblock",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    loopflow::journal::with_runtime(repo.path(), &["live-unblock-proof".into()], || {
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(home.path().join(format!(
                "runtime/exec-processes/{}.json",
                std::process::id()
            )))
            .unwrap(),
        )
        .unwrap();
        let owner: TaskWorkerOwner = serde_json::from_value(receipt).unwrap();
        let invocation =
            loopflow::engine::invocation::QueuedInvocation::load(repo.path(), "pursue").unwrap();
        let decision_index = invocation
            .steps
            .iter()
            .position(|step| {
                matches!(step, loopflow::engine::ConcreteStep::Skill(skill)
                    if skill.id.as_deref() == Some("decide"))
            })
            .expect("pursue has an implementation decision boundary");
        let position = FlowSession {
            invocation,
            cursor: loopflow::engine::ExecutionCursor {
                index: decision_index,
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
        };
        let (before, run) = runtime.block_on(async {
            let position = task
                .store
                .start_task_flow(&task.task.id, position)
                .await
                .unwrap();
            let TaskWorkerClaimOutcome::Claimed(claim) = task
                .store
                .claim_task_worker(
                    &task.task.id,
                    &position.invocation.id,
                    position.version,
                    &owner,
                    time::OffsetDateTime::now_utc(),
                )
                .await
                .unwrap()
            else {
                panic!("fixture claim")
            };
            // The step reserves its input after the worker acquires its claim.
            let reserved = task
                .store
                .reserve_attempt(position.id(), position.version, Some(&claim), None)
                .await
                .unwrap();
            let run = reserved.current_attempt.as_ref().unwrap().run_id.clone();
            task.store
                .publish_attempt(
                    &position.invocation.id,
                    reserved.version,
                reserved.current_attempt.as_ref().unwrap().captured,
                    Some(&claim),
                    "claude",
                    Some("sonnet"),
                )
                .await
                .unwrap();
            let before = task
                .store
                .task_flow(&task.task.id)
                .await
                .unwrap()
                .unwrap();
            (before, run)
        });
        let run_dir = home.path().join("runs").join(&run.strip_prefix("run_").unwrap_or(&run)[..2]).join(run.as_str());
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("events.jsonl"), format!("{}\n", serde_json::json!({
            "schema_version": 1, "seq": 0,
            "observed_at": time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            "type": "text", "text": "Fixture decision is active"
        }))).unwrap();
        // The deciding Run and its recovery share one keyed unblock Session.
        let keyed = |position: &FlowSession| {
            format!(
                "ask_once_{}",
                hex::encode(Sha256::digest(position.blocker_key().unwrap().as_bytes()))
            )
        };
        let session = keyed(&before);
        let ask_session = |session: &str, caller: String| loopflow::session::AgentSession {
            captured: None,
            id: session.to_string(), artifact_key: uuid::Uuid::new_v4().simple().to_string(), caller_artifact_key: Some(caller),
            input_published: true, cwd: repo.path().into(), skill: Some("unblock".into()),
            provider: Some("claude".into()), model: Some("sonnet".into()), node: None, iterations: None,
            task_id: Some(task.task.id.clone()), wave_id: Some(task.task.wave_id.clone()),
            flow_session_id: None, work_source: Some(loopflow::session::WorkSource::Inherited), bound_at: None,
            kind: loopflow::session::SessionKind::Ask, interactive: true, repo: None,
            title: "Choose a consumer".into(), title_source: loopflow::session::TitleSource::Generated,
            request: Some("Choose a consumer".into()), ready_summary: None, completed_at: None, created_at: 1,
        };
        let read = |args: &[&str]| {
            let output = Command::new(env!("CARGO_BIN_EXE_lf"))
                .args(args)
                .env("LF_DB_PATH", home.path().join("loopflow.db"))
                .env_remove("LF_WAVE_ID")
                .current_dir(repo.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
        };
        let status_args = ["task", "status", "INF-123", "--json"];
        // A waiting Ask from an earlier Run cannot repaint this worker.
        let stale = ask_session(&session, uuid::Uuid::new_v4().simple().to_string());
        let stale = runtime
            .block_on(task.store.create_session(stale, None))
            .unwrap();
        assert_eq!(read(&status_args)["execution"]["execution"]["state"], "running");
        // The same Session on the deciding Run blocks it, in the CLI and the desktop alike.
        let current = runtime
            .block_on(
                task.store
                    .replace_session_input(stale.captured, ask_session(&session, run.clone())),
            )
            .unwrap();
        let status = read(&status_args);
        let status = &status["execution"];
        assert_eq!(status["execution"]["state"], "blocked");
        assert_eq!(status["execution"]["captured"], before.current_attempt.as_ref().unwrap().captured);
        assert!(status["execution"]["reason"]
            .as_str()
            .unwrap()
            .contains(&session));
        let roadmap = read(&["roadmap", "--json"]);
        let snapshot = &roadmap["waves"][0]["tasks"]["items"][0];
        assert_eq!(snapshot["flow"]["record"]["execution"], "blocked");
        assert_eq!(
            snapshot["flow"]["record"]["reason"],
            status["execution"]["reason"]
        );
        let controls = snapshot["flow"]["controls"].as_array().unwrap();
        let resume = controls.iter().find(|c| c["kind"] == "resume").unwrap();
        assert!(resume["unavailable"]
            .as_str()
            .unwrap()
            .contains("without Task resume"));
        // Completion changes only observation; it neither releases the claim nor navigates.
        runtime
            .block_on(
                task.store
                    .ready_session(&session, current.captured, "Switch the reader"),
            )
            .unwrap();
        runtime
            .block_on(task.store.complete_session(&session, current.captured))
            .unwrap();
        assert_eq!(read(&status_args)["execution"]["execution"]["state"], "running");
        assert_eq!(
            runtime
                .block_on(task.store.task_flow(&task.task.id))
                .unwrap()
                .unwrap(),
            before
        );
        // Nor can the same Run's Ask from another boundary or invocation.
        for invocation_changed in [false, true] {
            let mut historical = before.clone();
            if invocation_changed {
                historical.invocation.id = "previous-invocation".into();
            } else {
                historical.cursor.iteration += 1;
            }
            let other = ask_session(&keyed(&historical), run.clone());
            runtime
                .block_on(task.store.create_session(other, None))
                .unwrap();
            assert_eq!(read(&status_args)["execution"]["execution"]["state"], "running");
        }
        // The body is real; the five-minute observation history is simulated.
        struct SleepingBody(std::process::Child);
        impl Drop for SleepingBody {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let body = SleepingBody(Command::new("sleep").arg("300").spawn().unwrap());
        let output = Command::new("ps").args(["-p", &body.0.id().to_string(), "-o", "ppid=,lstart=,time="]).env("LC_ALL", "C").output().unwrap();
        let output = String::from_utf8(output.stdout).unwrap();
        let fields = output.split_whitespace().collect::<Vec<_>>();
        let cpu_millis = (fields[6].split(':').fold(0.0, |seconds, part| seconds * 60.0 + part.parse::<f64>().unwrap()) * 1000.0).round() as u64;
        let now = time::OffsetDateTime::now_utc();
        let timestamp = |at: time::OffsetDateTime| at.format(&time::format_description::well_known::Rfc3339).unwrap();
        let observation = serde_json::json!({
            "schema_version": 1, "seq": 1, "observed_at": timestamp(now), "type": "activity",
            "observation": {"body_pid": body.0.id(), "quiet_since": timestamp(now - time::Duration::seconds(300)),
                "processes": [{"pid": body.0.id(), "parent": fields[0].parse::<u32>().unwrap(),
                    "started_at": fields[1..6].join(" "), "cpu_millis": cpu_millis}]}
        });
        std::fs::write(run_dir.join("events.jsonl"), format!("{observation}\n")).unwrap();
        let stalled = read(&status_args);
        let stalled = &stalled["execution"];
        let desktop = read(&["roadmap", "--json"]);
        drop(body);
        assert_eq!(stalled["execution"]["state"], "stalled");
        assert_eq!(stalled["execution"]["captured"], before.current_attempt.as_ref().unwrap().captured);
        let flow = &desktop["waves"][0]["tasks"]["items"][0]["flow"];
        assert_eq!(flow["record"]["execution"], "stalled");
        assert_eq!(flow["record"]["reason"], stalled["execution"]["reason"]);
        assert!(flow["controls"].as_array().unwrap().iter().find(|control| control["kind"] == "resume").unwrap()["unavailable"].as_str().unwrap().contains("Interrupt"));
        assert_eq!(runtime.block_on(task.store.task_flow(&task.task.id)).unwrap().unwrap(), before);
        Ok(())
    })
    .unwrap();
}

#[test]
fn initializing_worktree_keeps_status_wait_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/initializing-task";
    repo.create_branch(branch);
    let mut task = register_unrun_task(home.path(), repo.path(), branch, &base);
    let missing_worktree = home.path().join("not-yet-created-worktree");
    task.task.worktree = missing_worktree.clone();
    let runtime = tokio::runtime::Runtime::new().expect("initialization fixture runtime");
    runtime
        .block_on(task.store.update_task(&task.task))
        .expect("publish declared Task worktree");
    runtime
        .block_on(task.store.append_task_event(
            &task.task.id,
            &TaskEventKind::WorktreeInitializing {
                pr_id: task.pr.id.clone(),
                sequence: task.pr.sequence,
                branch: task.pr.branch.clone(),
                path: missing_worktree.display().to_string(),
                base_commit: task.pr.base_commit.clone(),
            },
        ))
        .expect("publish initialization marker");
    std::fs::create_dir_all(&missing_worktree)
        .expect("simulate a partially created worktree directory");
    let run_lf = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("LF_DB_PATH", home.path().join("loopflow.db"))
            .env_remove("LF_WAVE_ID")
            .current_dir(repo.path())
            .output()
            .expect("run lf read surface")
    };
    let status = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        status.status.success(),
        "status stays readable: {}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    let status = &status["execution"];
    assert_eq!(status["execution"]["state"], "idle");
    assert_eq!(status["work"]["sessions"], serde_json::json!([]));
    assert_eq!(status["work"]["flows"], serde_json::json!([]));
    assert_eq!(status["actions"]["recommended"], "no_action");
    assert!(status["actions"]["reason"]
        .as_str()
        .expect("status action reason")
        .contains("is initializing worktree"));

    let wait = run_lf(&["task", "wait", "INF-123", "--timeout", "0s", "--json"]);
    assert!(
        wait.status.success(),
        "wait stays readable: {}",
        String::from_utf8_lossy(&wait.stderr)
    );
    let wait: serde_json::Value = serde_json::from_slice(&wait.stdout).expect("wait JSON");
    assert_eq!(wait["actions"], status["actions"]);

    let roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        roadmap.status.success(),
        "roadmap stays readable: {}",
        String::from_utf8_lossy(&roadmap.stderr)
    );
    let roadmap: serde_json::Value = serde_json::from_slice(&roadmap.stdout).expect("roadmap JSON");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["tasks"]["state"], "ok", "roadmap wave: {wave:#}");
    let roadmap_task = &wave["tasks"]["items"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(roadmap_task["actions"]["recommended"], "no_action");
    assert!(roadmap_task["condition"]["reason"]
        .as_str()
        .expect("roadmap condition reason")
        .contains("is initializing worktree"));
    let projected = task_status(repo.path(), Some("INF-123"))
        .expect("read Task")
        .execution
        .expect("execution");
    assert_eq!(projected.actions.recommended, Some(TaskAction::NoAction));

    rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open stale initialization fixture")
        .execute(
            "UPDATE task_events SET created_at=?2 WHERE task_id=?1",
            rusqlite::params![
                task.task.id.as_str(),
                time::OffsetDateTime::now_utc().unix_timestamp() - 301,
            ],
        )
        .expect("age the initialization marker");
    let stale = run_lf(&["task", "status", "INF-123", "--json"]);
    assert!(
        stale.status.success(),
        "stale initialization stays readable"
    );
    let stale: serde_json::Value =
        serde_json::from_slice(&stale.stdout).expect("stale status JSON");
    let stale = &stale["execution"];
    assert_eq!(stale["actions"]["recommended"], "no_action");
    assert!(stale["actions"]["reason"]
        .as_str()
        .expect("stale action reason")
        .contains("initialization did not complete"));
    let stale_roadmap = run_lf(&["roadmap", "--wave", "task-pr-tests", "--json"]);
    assert!(
        stale_roadmap.status.success(),
        "stale roadmap stays readable"
    );
    let stale_roadmap: serde_json::Value =
        serde_json::from_slice(&stale_roadmap.stdout).expect("stale roadmap JSON");
    let stale_condition = &stale_roadmap["waves"][0]["tasks"]["items"][0]["condition"];
    assert_eq!(stale_condition["state"], "blocked");
    assert!(stale_condition["reason"]
        .as_str()
        .expect("stale roadmap reason")
        .contains("initialization did not complete"));
}

#[test]
fn missing_worktree_status_is_actionable_and_read_only() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let (task, missing_path, branch) = {
        let repo = TestRepo::new();
        let base = repo.head_sha();
        let branch = "jack/missing-worktree";
        repo.create_branch(branch);
        let task = register_unrun_task(home.path(), repo.path(), branch, &base);
        (task, repo.path().to_path_buf(), branch.to_string())
    };
    assert!(!missing_path.exists(), "fixture worktree is absent");
    let runtime = tokio::runtime::Runtime::new().expect("missing Task runtime");
    let before_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task before status")
        .expect("Task exists before status");
    let before_prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read PRs before status");

    let snapshot = task_status(&missing_path, Some("INF-123"))
        .expect("status survives the absent worktree")
        .execution
        .expect("execution");

    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot
        .actions
        .reason
        .contains(&missing_path.display().to_string()));
    assert!(snapshot.actions.reason.contains(&branch));
    assert!(snapshot.actions.reason.contains("lf task run INF-123"));
    assert!(snapshot
        .actions
        .reason
        .contains("identity and PR history are unchanged"));
    assert_eq!(
        runtime
            .block_on(task.store.get_task(&task.task.id))
            .expect("reread Task after status")
            .expect("Task remains registered"),
        before_task
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_prs(&task.task.id))
            .expect("reread PRs after status"),
        before_prs
    );
}
