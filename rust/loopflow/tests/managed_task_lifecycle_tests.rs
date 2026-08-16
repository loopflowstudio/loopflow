mod support;

use std::fs;
use std::process::Command;

use loopflow::child::ChildRef;
use loopflow::durable::WorkStatus;
use loopflow::ops::task::{task_resume, task_snapshot, task_status};
use loopflow::store::PmSnapshotRow;
use loopflow::task::actions::TaskAction;
use loopflow::task::{GithubPr, PrPublication, TaskEventKind, TaskLifecyclePlan, TaskPr, TaskPrId};
use loopflow_test_support::TestRepo;
use support::{register_task, EnvGuard, RegisteredTask};

fn tmux_script() -> &'static str {
    "#!/bin/sh\nexit 0\n"
}

fn configure_legacy_task(home: &std::path::Path, task: &mut RegisteredTask, identifier: &str) {
    task.task.plan.identifier = identifier.to_string();
    task.task.lifecycle = TaskLifecyclePlan::standard("task-kickoff", "task", "task-gate");
    task.task.phase_epoch = 2;
    task.task.phase_cursor = 3;
    task.task.phase_iteration = 4;
    task.task.gate_cycle = 1;
    task.task.provider_session_id = Some("provider-session-before-migration".to_string());
    let runtime = tokio::runtime::Runtime::new().expect("Task fixture runtime");
    runtime
        .block_on(task.store.update_task(&task.task))
        .expect("persist legacy Task lifecycle");
    let database = rusqlite::Connection::open(home.join("loopflow.db"))
        .expect("open legacy Task fixture store");
    database
        .execute(
            "UPDATE tasks SET phase_epoch=?2, phase_cursor=?3, phase_iteration=?4, gate_cycle=?5 \
             WHERE id=?1",
            rusqlite::params![
                task.task.id.as_str(),
                task.task.phase_epoch,
                task.task.phase_cursor,
                task.task.phase_iteration,
                task.task.gate_cycle,
            ],
        )
        .expect("persist the recorded lifecycle position");
    runtime
        .block_on(task.store.append_task_event(
            &task.task.id,
            &TaskEventKind::Failed {
                error: "task process failed: flow not found: task".to_string(),
                resumable: true,
            },
        ))
        .expect("record the missing-flow failure");
}

fn assert_legacy_resume_preserves_task(
    home: &std::path::Path,
    repo: &TestRepo,
    task: &RegisteredTask,
) {
    let runtime = tokio::runtime::Runtime::new().expect("read Task before migration");
    let expected_prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .expect("read original PR chain");
    let dirty = repo.path().join("dirty-uncommitted.txt");
    fs::write(&dirty, "preserve this exact local work\n").expect("write dirty Task work");

    let before = task_status(&task.task.plan.identifier).expect("read parked legacy Task");
    let snapshot = task_snapshot(&before).expect("project legacy Task status");
    assert_eq!(snapshot.lifecycle.loop_.flow, "task");
    assert_eq!(snapshot.actions.recommended, Some(TaskAction::Resume));
    assert!(snapshot
        .actions
        .reason
        .contains("migrating retired loop flow"));
    assert!(snapshot.actions.reason.contains("\"task\" to \"slice\""));
    assert!(matches!(snapshot.status, WorkStatus::Ready));
    assert_eq!(
        snapshot.latest_event.as_ref().map(|event| &event.kind),
        Some(&TaskEventKind::Failed {
            error: "task process failed: flow not found: task".to_string(),
            resumable: true,
        })
    );
    let after_status = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("reread Task after status")
        .expect("Task remains registered after status");
    assert_eq!(
        after_status.lifecycle.loop_.flow, "task",
        "status advertises the explicit migration without applying it"
    );

    task_resume(&task.task.plan.identifier, None, None)
        .expect("explicit resume migrates before launch");

    let migrated = runtime
        .block_on(task.store.get_task(&task.task.id))
        .expect("read Task")
        .expect("same Task remains registered");
    assert_eq!(migrated.id, task.task.id);
    assert_eq!(migrated.lifecycle.first.flow, "task-kickoff");
    assert_eq!(migrated.lifecycle.loop_.flow, "slice");
    assert_eq!(migrated.lifecycle.finally.flow, "task-gate");
    assert_eq!(migrated.lifecycle_phase, task.task.lifecycle_phase);
    assert_eq!(migrated.phase_epoch, task.task.phase_epoch);
    assert_eq!(migrated.phase_cursor, task.task.phase_cursor);
    assert_eq!(migrated.phase_iteration, task.task.phase_iteration);
    assert_eq!(migrated.gate_cycle, task.task.gate_cycle);
    assert_eq!(
        migrated.provider_session_id, task.task.provider_session_id,
        "provider history remains attached to the same Task"
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_prs(&task.task.id))
            .expect("read preserved PR chain"),
        expected_prs
    );
    assert_eq!(
        fs::read_to_string(&dirty).expect("read preserved dirty work"),
        "preserve this exact local work\n"
    );
    let work = runtime
        .block_on(
            task.store
                .work_for_child(&ChildRef::Task(task.task.id.clone())),
        )
        .expect("resolve Task Work");
    assert!(matches!(
        runtime.block_on(task.store.work_status(&work)).unwrap(),
        WorkStatus::Running { .. }
    ));

    let database = rusqlite::Connection::open(home.join("loopflow.db")).expect("open Task store");
    let run_count: i64 = database
        .query_row(
            "SELECT count(*) FROM runs r JOIN epochs e ON e.id=r.epoch_id WHERE e.task_id=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .expect("count Task Runs");
    assert_eq!(
        run_count, 2,
        "one historical Run plus one repaired resume Run"
    );
    let resume_token: String = database
        .query_row(
            "SELECT ai.resume_token FROM agent_invocations ai \
             JOIN runs r ON r.id=ai.run_id JOIN epochs e ON e.id=r.epoch_id \
             WHERE e.task_id=?1 AND ai.resume_token IS NOT NULL \
             ORDER BY ai.started_at DESC LIMIT 1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .expect("read repaired provider continuation");
    assert_eq!(resume_token, "provider-session-before-migration");
}

#[test]
fn parked_dirty_legacy_task_resumes_through_an_explicit_migration() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[("tmux", tmux_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/loo-193-proof";
    repo.create_branch(branch);
    let mut task = register_task(home.path(), repo.path(), branch, &base);
    configure_legacy_task(home.path(), &mut task, "LOO-193");
    assert_legacy_resume_preserves_task(home.path(), &repo, &task);
}

#[test]
fn legacy_migration_preserves_merged_prs_and_the_open_successor() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[("tmux", tmux_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let first_branch = "jack/legacy-first";
    repo.create_branch(first_branch);
    let mut task = register_task(home.path(), repo.path(), first_branch, &base);
    configure_legacy_task(home.path(), &mut task, "LOO-150");

    let now = time::OffsetDateTime::now_utc();
    let mut merged = task.pr.clone();
    merged.publication = Some(PrPublication {
        requested_at: now,
        github: Some(GithubPr {
            number: 912,
            url: "https://example.com/pr/912".to_string(),
            head_sha: Some(repo.head_sha()),
        }),
        merge: None,
    });
    merged.merge_commit = Some("merged-912".to_string());
    merged.updated_at = now;
    let successor_branch = "jack/legacy-successor";
    repo.create_branch(successor_branch);
    let successor = TaskPr {
        id: TaskPrId::new(),
        task_id: task.task.id.clone(),
        sequence: 2,
        slug: "legacy-successor".to_string(),
        branch: successor_branch.to_string(),
        base_commit: repo.head_sha(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    let runtime = tokio::runtime::Runtime::new().expect("settle predecessor");
    runtime
        .block_on(task.store.settle_task_pr(&merged, Some(&successor)))
        .expect("record merged PR and its active successor");
    assert_legacy_resume_preserves_task(home.path(), &repo, &task);
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
        let task = register_task(home.path(), repo.path(), branch, &base);
        (task, repo.path().to_path_buf(), branch.to_string())
    };
    assert!(!missing_path.exists(), "fixture worktree is absent");
    let runtime = tokio::runtime::Runtime::new().expect("missing Task runtime");
    let before_task = runtime
        .block_on(task.store.get_task(&task.task.id))
        .unwrap()
        .unwrap();
    let before_prs = runtime
        .block_on(task.store.task_prs(&task.task.id))
        .unwrap();

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
            .unwrap()
            .unwrap(),
        before_task
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_prs(&task.task.id))
            .unwrap(),
        before_prs
    );
}

#[test]
fn unknown_missing_flow_never_becomes_a_run_or_legal_resume() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[("tmux", tmux_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/unknown-flow";
    repo.create_branch(branch);
    let mut task = register_task(home.path(), repo.path(), branch, &base);
    task.task.lifecycle.loop_.flow = "removed-custom-flow".to_string();
    let runtime = tokio::runtime::Runtime::new().expect("invalid lifecycle runtime");
    runtime
        .block_on(task.store.update_task(&task.task))
        .expect("persist the historical invalid pin");

    let status = task_status("INF-123").expect("invalid lifecycle remains inspectable");
    let snapshot = task_snapshot(&status).expect("project invalid lifecycle status");
    assert_eq!(snapshot.actions.recommended, Some(TaskAction::NoAction));
    assert!(snapshot.actions.reason.contains("removed-custom-flow"));
    assert!(snapshot.actions.reason.contains("restore the pinned flow"));
    for _ in 0..2 {
        let error = task_resume("INF-123", None, None)
            .expect_err("unknown migration must fail before launch")
            .to_string();
        assert!(error.contains("removed-custom-flow"));
    }
    let database = rusqlite::Connection::open(home.path().join("loopflow.db"))
        .expect("open invalid Task store");
    let run_count: i64 = database
        .query_row(
            "SELECT count(*) FROM runs r JOIN epochs e ON e.id=r.epoch_id WHERE e.task_id=?1",
            [task.task.id.as_str()],
            |row| row.get(0),
        )
        .expect("count invalid Task Runs");
    assert_eq!(run_count, 1, "only the fixture's historical Run exists");
}

#[test]
fn initializing_worktree_keeps_status_wait_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("Task home");
    let _env = EnvGuard::with_lf_home(&[("tmux", tmux_script())], home.path());
    let repo = TestRepo::new();
    let base = repo.head_sha();
    let branch = "jack/initializing-task";
    repo.create_branch(branch);
    let mut task = register_task(home.path(), repo.path(), branch, &base);
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
    let project = runtime
        .block_on(task.store.get_project(&task.task.project_id))
        .expect("read owning Project")
        .expect("owning Project exists");
    let payload = serde_json::json!({
        "projects": [{
            "id": project.plan.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "summary": project.plan.prompt_context,
            "definition": project.plan.prompt_context,
            "flows": {"first": null, "loop": null, "finally": null},
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
        .block_on(
            task.store.put_pm_snapshot(PmSnapshotRow {
                repo: std::fs::canonicalize(repo.path())
                    .expect("canonical Wave repository")
                    .display()
                    .to_string(),
                wave: "task-pr-tests".to_string(),
                provider: "linear".to_string(),
                initiative: "initialization-initiative".to_string(),
                synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
            }),
        )
        .expect("seed roadmap planning");

    let run_lf = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env_remove("LF_DB_PATH")
            .env_remove("LF_WAVE_ID")
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
    assert_eq!(wave["projects"]["state"], "ok");
    let roadmap_task = &wave["projects"]["items"][0]["tasks"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "INF-123");
    assert_eq!(
        roadmap_task["attention"]["actions"]["recommended"],
        "no_action"
    );
    assert!(roadmap_task["attention"]["reason"]
        .as_str()
        .expect("roadmap attention reason")
        .contains("is initializing worktree"));
    let work = runtime
        .block_on(
            task.store
                .work_for_child(&ChildRef::Task(task.task.id.clone())),
        )
        .expect("resolve initializing Task Work");
    assert!(runtime
        .block_on(task.store.current_run(&work))
        .expect("read initializing Task Run")
        .is_none());
}
