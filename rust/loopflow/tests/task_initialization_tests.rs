#[path = "support/installation.rs"]
mod installation;
mod support;

use std::fs;
use std::process::Command;
use std::time::Duration;

use loopflow::machine_install::{
    self, ActivationTargets, ArtifactRole, MachineInstallState, RecoveryOwner, SwitchPhase,
    SwitchReceipt,
};
use loopflow::ops::task::{task_snapshot, task_status};
use loopflow::ops::task_actions::TaskAction;
use loopflow::store::PmSnapshotRow;
use loopflow::work::task::TaskEventKind;
use loopflow_test_support::TestRepo;
use rusqlite::{backup::Backup, Connection, OpenFlags};
use sha2::{Digest, Sha256};
use support::{register_unrun_task, EnvGuard};

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
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LF_") {
                command.env_remove(name);
            }
        }
        command
            .current_dir(&invoking)
            .args(["task", "checkout", "INF-123", "--json"])
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

fn retain_installation_and_select_store(store: &std::path::Path) {
    let root = machine_install::root().unwrap();
    let MachineInstallState::Settled(mut active) = machine_install::read_state(&root).unwrap()
    else {
        panic!("fixture has a settled installation");
    };
    let receipt = SwitchReceipt {
        schema_version: 1,
        id: "task-proof-retained".into(),
        prior: Some(active.selection.clone()),
        target: active.selection.clone(),
        published_fallback: Some(active.published_fallback.clone()),
        target_published_fallback: None,
        phase: SwitchPhase::Settled,
        recovery_owner: RecoveryOwner::Candidate,
        target_store_advance_started: true,
        target_store_advanced: true,
        active_selection_committed: true,
        coordinator: active
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone(),
        candidate: active
            .selection
            .artifact_set
            .artifact(&ArtifactRole::Cli)
            .unwrap()
            .clone(),
        activation: ActivationTargets {
            cli: root.join("lf"),
            daemon: None,
            app: None,
            legacy_app: None,
        },
        app_was_running: false,
        disposable_store_owned: false,
    };
    receipt.validate().unwrap();
    fs::create_dir_all(root.join("receipts")).unwrap();
    fs::write(
        root.join("receipts/task-proof-retained.json"),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    active.selection.installation_id = "task-proof-later".into();
    active.selection.store = store.to_path_buf();
    active.validate().unwrap();
    // Author historical installation evidence; this fixture does not run promotion.
    fs::write(
        root.join("active.json"),
        serde_json::to_vec(&active).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn incompatible_branch_data_recommends_only_a_verified_retained_pair() {
    let home = tempfile::tempdir().unwrap();
    let private_home = tempfile::tempdir().unwrap();
    let later_home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let installed_db = home.path().join("loopflow.db");
    let private_db = private_home.path().join("loopflow.db");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    drop(
        runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(installed_db.clone()),
            ))
            .unwrap(),
    );
    let installation = installation::Installation::new(home.path());
    let source =
        Connection::open_with_flags(&installed_db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut private = Connection::open(&private_db).unwrap();
    Backup::new(&source, &mut private)
        .unwrap()
        .run_to_completion(4096, Duration::from_millis(10), None)
        .unwrap();
    let sql = "CREATE TABLE future_notes (note TEXT);";
    let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
    private.execute_batch(sql).unwrap();
    private
        .execute(
            "INSERT INTO future_notes VALUES ('independent branch work')",
            [],
        )
        .unwrap();
    private
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS development_migrations (
            position INTEGER NOT NULL UNIQUE, id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL
        );",
        )
        .unwrap();
    private.execute(
        "INSERT INTO development_migrations SELECT COUNT(*), '33333333333333333333333333333333', 'future_notes', ?1, 1 FROM development_migrations",
        [&checksum],
    ).unwrap();
    private
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    drop(private);
    drop(source);
    let bytes = || {
        [
            std::fs::read(&installed_db).unwrap(),
            std::fs::read(&private_db).unwrap(),
        ]
    };
    let refuse = || {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["home", "id", "--json"])
            .env("LF_HOME", private_home.path())
            .env("LF_DB_PATH", &private_db)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let message = String::from_utf8(output.stderr).unwrap();
        assert!(
            message.contains(private_home.path().to_str().unwrap()),
            "{message}"
        );
        assert!(message.contains(private_db.to_str().unwrap()), "{message}");
        assert!(message.contains("future_notes"), "{message}");
        assert!(message.contains(&checksum), "{message}");
        assert!(!message.contains("--fresh"), "{message}");
        message
    };
    let before = bytes();
    let message = refuse();
    assert!(message.contains("Verified retained pair:"), "{message}");
    assert!(
        message.contains(installation.cli.to_str().unwrap()),
        "{message}"
    );
    assert!(
        message.contains(installed_db.to_str().unwrap()),
        "{message}"
    );
    assert_eq!(bytes(), before);

    // A historical pair remains recoverable even when the current Home is newer.
    let later_db = later_home.path().join("loopflow.db");
    drop(
        runtime
            .block_on(loopflow::store::open_ephemeral_store(
                &loopflow::store::StorageConfig::sqlite(later_db.clone()),
            ))
            .unwrap(),
    );
    let later = Connection::open(&later_db).unwrap();
    later
        .execute_batch("CREATE TABLE newer_installed_data (note TEXT);")
        .unwrap();
    drop(later);
    let later_bytes = std::fs::read(&later_db).unwrap();
    retain_installation_and_select_store(&later_db);
    let message = refuse();
    assert!(message.contains("Verified retained pair:"), "{message}");
    assert!(message.contains("--reuse-home 'task-proof'"), "{message}");
    assert!(message.contains("--cli-target"), "{message}");
    assert!(!message.contains("--daemon-target"), "{message}");
    assert!(message.contains("--preview"), "{message}");
    assert_eq!(bytes(), before);
    assert_eq!(std::fs::read(&later_db).unwrap(), later_bytes);

    // The same retained bytes must not be recommended after their database changes.
    let changed = Connection::open(&installed_db).unwrap();
    changed
        .execute_batch("CREATE TABLE unrecognized_installed_data (note TEXT);")
        .unwrap();
    drop(changed);
    let before = bytes();
    let message = refuse();
    assert!(
        message.contains("No compatible retained executable/database pair was verified"),
        "{message}"
    );
    assert!(!message.contains("Verified retained pair:"), "{message}");
    assert_eq!(bytes(), before);

    std::fs::rename(
        &installation.cli,
        installation.cli.with_extension("retained"),
    )
    .unwrap();
    let message = refuse();
    assert!(
        message.contains("No compatible retained executable/database pair was verified"),
        "{message}"
    );
    assert_eq!(bytes(), before);
    let private =
        Connection::open_with_flags(&private_db, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        private
            .query_row("SELECT note FROM future_notes", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "independent branch work"
    );
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn task_review_completion_consumes_only_installed_readiness() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let repo = TestRepo::new();
    repo.create_branch("jack/review-installation");
    let task = register_unrun_task(
        home.path(),
        repo.path(),
        "jack/review-installation",
        &repo.head_sha(),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let position = loopflow::durable::FlowSession {
        task_id: Some(task.task.id.clone()),
        wave_id: Some(task.task.wave_id.clone()),
        cwd: task.task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        finished: false,
        invocation: loopflow::engine::invocation::QueuedInvocation::load(
            repo.path(),
            "task-design",
        )
        .unwrap(),
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
    let position = runtime
        .block_on(task.store.start_task_flow(&task.task.id, position))
        .unwrap();
    // This proof consumes an existing review's readiness. Seed its acknowledged
    // launch, without starting a terminal or provider in the installation fixture.
    let position = runtime
        .block_on(
            task.store
                .reserve_task_review(position.id(), position.version),
        )
        .unwrap();
    let (_, review) = runtime
        .block_on(task.store.reserve_review_run(&position))
        .unwrap();
    assert_eq!(
        Connection::open(home.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET input_published=1, provider='claude', model='sonnet' WHERE current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)",
                [review.artifact_key.as_str()],
            )
            .unwrap(),
        1
    );
    let installation = installation::Installation::new(home.path());
    let command = |cli: &std::path::Path, selected_home: &std::path::Path, args: &[&str]| {
        let mut command = Command::new(cli);
        command
            .args(args)
            .current_dir(repo.path())
            .env("LF_HOME", selected_home)
            .env("LF_DB_PATH", selected_home.join("loopflow.db"));
        command
    };
    let branch = std::path::Path::new(env!("CARGO_BIN_EXE_lf"));
    // Inheriting installed data must seed a private copy, then run against installation.
    let started = command(
        branch,
        home.path(),
        &["-m", "claude:sonnet", "task", "run", "INF-123", "--json"],
    )
    .env("LF_BIN", branch)
    .output()
    .unwrap();
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&started.stdout).unwrap();
    assert_eq!(snapshot["agent"], "claude:sonnet");
    assert_eq!(snapshot["provider"], "claude");
    let report = String::from_utf8(started.stderr).unwrap();
    let branch_data = std::path::Path::new(
        report
            .lines()
            .find_map(|line| {
                line.strip_prefix("Branch lf is using data directory ")?
                    .split_once("; installed store")
                    .map(|(path, _)| path)
            })
            .expect("branch reports its private copy"),
    );
    let position = runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .unwrap();
    assert!(position.review_artifact_key().is_some());
    let repeated = command(branch, home.path(), &["task", "run", "INF-123", "--json"])
        .output()
        .unwrap();
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let resumed = runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .unwrap();
    assert_eq!(resumed.invocation, position.invocation);
    assert_eq!(resumed.cursor, position.cursor);
    assert_eq!(
        resumed.review_artifact_key(),
        position.review_artifact_key()
    );
    for (cli, expected) in [
        (
            installation.cli.as_path(),
            serde_json::json!("claude:sonnet"),
        ),
        (branch, serde_json::Value::Null),
    ] {
        let status = command(cli, home.path(), &["task", "status", "INF-123", "--json"])
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(status["agent"], expected);
    }
    // The private snapshot has no installed input bundle or completion authority.
    let branch_store = runtime
        .block_on(loopflow::store::open_store(
            &loopflow::store::StorageConfig::sqlite(branch_data.join("loopflow.db")),
        ))
        .unwrap();
    let session_id = position.pending_session_id.as_ref().unwrap();
    let session = runtime
        .block_on(task.store.session(session_id))
        .unwrap()
        .unwrap();
    runtime
        .block_on(branch_store.ready_session(
            &session.id,
            session.captured,
            "Branch-only feedback must stay private",
        ))
        .unwrap();
    let copied = runtime
        .block_on(branch_store.task_flow(&task.task.id))
        .unwrap()
        .unwrap();
    let branch_events = runtime
        .block_on(branch_store.task_events_after(&task.task.id, 0))
        .unwrap();
    // A bad replacement delegates first, then refuses before any installed
    // refresh, checkpoint, or stop. Neither copy loses its saved review.
    let head = repo.head_sha();
    let rejected = command(
        branch,
        branch_data,
        &["task", "restart", "INF-123", "--flow", "missing-flow"],
    )
    .output()
    .unwrap();
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("missing-flow"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_flow(&task.task.id))
            .unwrap()
            .unwrap(),
        resumed
    );
    assert_eq!(repo.head_sha(), head, "no restart checkpoint was committed");
    let step = position.current();
    let node = step.id.as_ref().unwrap();
    let boundary = format!(
        "{}:{}:{}:{}:{}",
        task.task.id, position.invocation.id, step.flow, node, position.cursor.iteration
    );
    let complete = |id: &str| {
        command(branch, branch_data, &["session", "complete", id])
            .output()
            .unwrap()
    };
    let rejected = complete(&boundary);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("has not marked this ready"));
    assert_eq!(
        runtime
            .block_on(task.store.task_flow(&task.task.id))
            .unwrap()
            .unwrap(),
        position
    );
    assert_eq!(
        runtime
            .block_on(branch_store.task_flow(&task.task.id))
            .unwrap()
            .unwrap(),
        copied
    );

    let loopflow::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
        panic!("review skill")
    };
    let mut token = serde_json::json!({"kind": "flow", "token": {
        "task_id": task.task.id, "invocation_id": position.invocation.id,
        "flow": step.flow, "node_id": node, "skill": planned.skill,
        "iteration": position.cursor.iteration + 1,
    }});
    let ready = |token: &serde_json::Value| {
        command(
            &installation.cli,
            home.path(),
            &["session", "ready", "Installed feedback"],
        )
        .env(
            "LF_RUN_ID",
            position.review_artifact_key().unwrap().as_str(),
        )
        .env("LF_HUMAN_SESSION", token.to_string())
        .output()
        .unwrap()
    };
    let stale = ready(&token);
    assert!(!stale.status.success());
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("Session is stale"),
        "{}",
        String::from_utf8_lossy(&stale.stderr)
    );
    assert_eq!(
        runtime
            .block_on(task.store.task_flow(&task.task.id))
            .unwrap()
            .unwrap(),
        position
    );
    token["token"]["iteration"] = position.cursor.iteration.into();
    let ready = ready(&token);
    assert!(
        ready.status.success(),
        "{}",
        String::from_utf8_lossy(&ready.stderr)
    );
    // Both public selectors must resolve the same installed review boundary.
    let accepted = complete(position.review_artifact_key().unwrap().as_str());
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    assert!(runtime
        .block_on(task.store.task_flow(&task.task.id))
        .unwrap()
        .is_none());
    let events = runtime
        .block_on(task.store.task_events_after(&task.task.id, 0))
        .unwrap();
    let finished: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.kind {
            TaskEventKind::FlowFinished {
                invocation_id,
                summary,
                ..
            } => Some((invocation_id.as_str(), summary.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        finished,
        [(position.invocation.id.as_str(), "Installed feedback")]
    );
    assert_eq!(
        runtime
            .block_on(branch_store.task_flow(&task.task.id))
            .unwrap()
            .unwrap(),
        copied
    );
    assert_eq!(
        runtime
            .block_on(branch_store.task_events_after(&task.task.id, 0))
            .unwrap(),
        branch_events
    );
}

#[test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
fn direct_open_preserves_another_installations_development_store() {
    let home = tempfile::tempdir().unwrap();
    let _env = EnvGuard::with_lf_home(&[], home.path());
    let database = home.path().join("loopflow.db");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &loopflow::store::StorageConfig::sqlite(database.clone()),
        ))
        .unwrap();
    let _installation = installation::Installation::new(home.path());
    let bytes = || {
        ["loopflow.db", "loopflow.db-wal", "loopflow.db-shm"]
            .map(|name| std::fs::read(home.path().join(name)).ok())
    };
    let before = bytes();
    let error = loopflow::store::sqlite::SqliteStore::new(&database).unwrap_err();
    assert!(error
        .to_string()
        .contains("belongs to another installation"));
    assert_eq!(bytes(), before);
    drop(store);
}

#[test]
fn task_live_unblock_status_and_desktop_share_exact_boundary_and_recovery() {
    use loopflow::durable::{FlowSession, TaskWorkerClaimOutcome, TaskWorkerOwner};
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
        assert_eq!(read(&status_args)["execution"]["state"], "running");
        // The same Session on the deciding Run blocks it, in the CLI and the desktop alike.
        let current = runtime
            .block_on(
                task.store
                    .replace_session_input(stale.captured, ask_session(&session, run.clone())),
            )
            .unwrap();
        let status = read(&status_args);
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
        assert_eq!(read(&status_args)["execution"]["state"], "running");
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
            "metric_targets": [],
            "flow": "feature", "status": "started",
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
    let projected = task_snapshot(&task_status(Some("INF-123")).expect("read Task"))
        .expect("project Task status");
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

    let status = task_status(Some("INF-123")).expect("status survives the absent worktree");
    let snapshot = task_snapshot(&status).expect("project missing-worktree status");

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
