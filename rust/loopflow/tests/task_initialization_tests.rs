#[path = "support/chapter.rs"]
mod chapter;
mod support;

use std::process::Command;

use loopflow::ops::task::{task_snapshot, task_status};
use loopflow::ops::task_actions::TaskAction;
use loopflow::store::PmSnapshotRow;
use loopflow::work::task::TaskEventKind;
use loopflow_test_support::TestRepo;
use support::{register_unrun_task, EnvGuard};

#[test]
fn task_live_unblock_status_and_desktop_share_exact_boundary_and_recovery() {
    use loopflow::durable::{FlowPosition, RunId, TaskWorkerClaimOutcome, TaskWorkerOwner};
    use sha2::{Digest, Sha256};
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
        let position = FlowPosition {
            task_id: task.task.id.clone(),
            invocation: loopflow::engine::invocation::QueuedInvocation::load(repo.path(), "pursue")
                .unwrap(),
            session_run_id: None,
            ready_summary: None,
            cursor: loopflow::engine::ExecutionCursor {
                index: 3,
                ..Default::default()
            },
            version: 0,
            worker_generation: 0,
            claim: None,
            failure: None,
            updated_at: time::OffsetDateTime::now_utc(),
        };
        let run = RunId::new();
        let run_dir = home.path().join("runs").join(&run.as_str()[4..6]).join(run.as_str());
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("events.jsonl"), format!("{}\n", serde_json::json!({
            "schema_version": 1, "seq": 0,
            "observed_at": time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap(),
            "type": "text", "text": "Fixture decision is active"
        }))).unwrap();
        let before = runtime.block_on(async {
            let position = task
                .store
                .set_flow_position(&task.task.id, position)
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
            task.store
                .bind_task_worker_run(&task.task.id, &claim, &run, &owner)
                .await
                .unwrap();
            task.store
                .flow_position(&task.task.id)
                .await
                .unwrap()
                .unwrap()
        });
        let key = format!(
            "task:{}:{}:{}",
            task.task.id,
            before.invocation.id,
            before.cursor.boundary_key()
        );
        let session = format!("ask_once_{}", hex::encode(Sha256::digest(key.as_bytes())));
        let record_path = home
            .path()
            .join("human-sessions")
            .join(format!("{session}.json"));
        std::fs::create_dir_all(record_path.parent().unwrap()).unwrap();
        let mut record = serde_json::json!({
            "id": session, "parent_run_id": run, "parent_run_dir": null,
            "work": null, "work_selector": null, "title": "Choose a consumer",
            "detail": "Fixture decision wait", "prompt": "Choose a consumer",
            "skill": "unblock", "cwd": repo.path(), "model": "claude:sonnet",
            "session_run_id": null, "ready_summary": null, "status": "waiting",
            "retain_completed": true
        });
        let write = |record: &serde_json::Value| {
            std::fs::write(&record_path, serde_json::to_vec(record).unwrap()).unwrap()
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
        write(&record);
        let status_args = ["task", "status", "INF-123", "--json"];
        let status = read(&status_args);
        assert_eq!(status["execution"]["state"], "blocked");
        assert_eq!(status["execution"]["run_id"], run.as_str());
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
        record["status"] = serde_json::json!({"completed": {"summary": "Switch the reader"}});
        write(&record);
        assert_eq!(read(&status_args)["execution"]["state"], "running");
        assert_eq!(
            runtime
                .block_on(task.store.flow_position(&task.task.id))
                .unwrap()
                .unwrap(),
            before
        );
        // A waiting Ask from an earlier Run cannot repaint this worker.
        record["status"] = serde_json::json!("waiting");
        record["parent_run_id"] = serde_json::json!(RunId::new());
        write(&record);
        assert_eq!(read(&status_args)["execution"]["state"], "running");
        // Nor can the same Run's Ask from another boundary or invocation.
        std::fs::remove_file(&record_path).unwrap();
        record["parent_run_id"] = serde_json::json!(run);
        for invocation_changed in [false, true] {
            let mut historical = before.clone();
            if invocation_changed {
                historical.invocation.id = "previous-invocation".into();
            } else {
                historical.cursor.iteration += 1;
            }
            let key = format!(
                "task:{}:{}:{}",
                task.task.id,
                historical.invocation.id,
                historical.cursor.boundary_key()
            );
            let id = format!("ask_once_{}", hex::encode(Sha256::digest(key.as_bytes())));
            record["id"] = serde_json::json!(id);
            std::fs::write(
                record_path.parent().unwrap().join(format!("{id}.json")),
                serde_json::to_vec(&record).unwrap(),
            )
            .unwrap();
            assert_eq!(read(&status_args)["execution"]["state"], "running");
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
        let desktop = read(&["roadmap", "--json"]);
        drop(body);
        assert_eq!(stalled["execution"]["state"], "stalled");
        assert_eq!(stalled["execution"]["run_id"], run.as_str());
        let flow = &desktop["waves"][0]["tasks"]["items"][0]["flow"];
        assert_eq!(flow["record"]["execution"], "stalled");
        assert_eq!(flow["record"]["reason"], stalled["execution"]["reason"]);
        assert!(flow["controls"].as_array().unwrap().iter().find(|control| control["kind"] == "resume").unwrap()["unavailable"].as_str().unwrap().contains("Interrupt"));
        assert_eq!(runtime.block_on(task.store.flow_position(&task.task.id)).unwrap().unwrap(), before);
        Ok(())
    })
    .unwrap();
}

#[test]
fn task_agent_cli_resume_persists_choice_and_reports_it_on_later_reads() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_branch("jack/task-agent");
    let task = register_unrun_task(
        home.path(),
        repo.path(),
        "jack/task-agent",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let position = loopflow::durable::FlowPosition {
        task_id: task.task.id.clone(),
        invocation: loopflow::engine::invocation::QueuedInvocation::load(
            repo.path(),
            "task-design",
        )
        .unwrap(),
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
    };
    runtime
        .block_on(task.store.set_flow_position(&task.task.id, position))
        .unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("LF_DB_PATH", home.path().join("loopflow.db"))
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
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
    assert!(run(&["task", "status", "INF-123", "--json"])["agent"].is_null());
    run(&["-m", "claude:sonnet", "task", "resume", "INF-123", "--json"]);
    run(&["task", "resume", "INF-123", "--json"]);
    let status = run(&["task", "status", "INF-123", "--json"]);
    assert_eq!(status["agent"], "claude:sonnet");
    assert_eq!(status["provider"], "claude");
    assert_eq!(
        runtime
            .block_on(task.store.get_task(&task.task.id))
            .unwrap()
            .unwrap()
            .agent
            .as_deref(),
        Some("claude:sonnet")
    );
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
    let project = runtime
        .block_on(task.store.get_project(&task.task.project_id))
        .expect("read owning Project")
        .expect("owning Project exists");
    runtime
        .block_on(task.store.save_chapter(
            &chapter::current_chapter(
                &task.task.wave_id,
                "task-pr-tests",
                project.plan.id.as_str(),
            ),
            true,
        ))
        .expect("bind current chapter");
    let payload = serde_json::json!({
        "projects": [{
            "id": project.plan.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "summary": project.plan.prompt_context,
            "metric_targets": [],
            "flows": {"recommended": null},
            "krs": [],
            "initiative_ids": ["initialization-initiative"],
            "team_ids": ["initialization-team"]
        }],
        "items": [{
            "id": task.task.plan.id.as_str(),
            "identifier": task.task.plan.identifier,
            "url": null,
            "name": task.task.plan.title,
            "description": task.task.plan.description,
            "rank": 1,
            "completed": false,
            "project_id": project.plan.id.as_str(),
            "project": project.plan.slug,
            "team_id": "initialization-team",
            "assignee": null
        }]
    });
    runtime
        .block_on(task.store.put_pm_snapshot(PmSnapshotRow {
            wave_id: task.task.wave_id.clone(),
            provider: "linear".to_string(),
            initiative: "initialization-initiative".to_string(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
        }))
        .expect("seed roadmap planning");
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
    assert_eq!(status["execution"]["state"], "idle");
    assert_eq!(status["runs"], serde_json::json!([]));
    assert_eq!(status["runs_truncated"], false);
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
    let projected =
        task_snapshot(&task_status("INF-123").expect("read Task")).expect("project Task status");
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

    let status = task_status("INF-123").expect("status survives the absent worktree");
    let snapshot = task_snapshot(&status).expect("project missing-worktree status");

    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot
        .actions
        .reason
        .contains(&missing_path.display().to_string()));
    assert!(snapshot.actions.reason.contains(&branch));
    assert!(snapshot.actions.reason.contains("lf task resume INF-123"));
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
