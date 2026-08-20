use std::path::Path;
use std::process::Command;

use loopflow::durable::{
    AdvanceReceipt, Containment, ContainmentObservation, HomeId, InvocationRoute, RunAdvance,
    RunTrigger, StopCause, WorkRef,
};
use loopflow::engine::wave_context::{resolve_managed_wave, WaveResolveError};
use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::project::{Project, ProjectId};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_store, PmSnapshotRow, StorageConfig};
use loopflow::task::{
    Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
    TaskPrId,
};
use loopflow::trace::{AgentInvocationRow, AgentTurnRow, SupervisedInvocation};
use loopflow::wave::journal;
use loopflow::wave::relocate::relocate_wave;
use loopflow::wave::{Wave, WaveLocator};
use time::OffsetDateTime;

fn repository(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    let output = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(path)
        .output()
        .unwrap();
    assert!(output.status.success());
}

fn author_wave(repo: &Path, slug: &str, marker: &str) {
    let wave = repo.join("wave").join(slug);
    std::fs::create_dir_all(&wave).unwrap();
    std::fs::write(wave.join("GOAL.md"), format!("# {marker}\n")).unwrap();
    let journal = journal::journal_path(repo, slug);
    std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
    std::fs::write(journal, format!("{{\"repository\":\"{marker}\"}}\n")).unwrap();
}

fn registered_wave(repo: &Path, slug: &str) -> Wave {
    let locator = WaveLocator::discover(repo, slug).unwrap();
    Wave::new(
        WaveId::new(),
        locator.slug().to_string(),
        locator.repo().to_string(),
    )
}

fn project(wave: &Wave) -> Project {
    let now = OffsetDateTime::now_utc();
    Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("project-alpha").unwrap(),
            slug: "architecture".to_string(),
            name: "Architecture".to_string(),
            prompt_context: "Keep identity boring.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        iteration: 1,
        observation_cursor: 0,
        last_state_fingerprint: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    }
}

fn task(wave: &Wave, project: &Project, repo: &Path) -> (Task, TaskPr) {
    let now = OffsetDateTime::now_utc();
    let id = TaskId::new();
    let task = Task {
        id: id.clone(),
        plan: TaskPlan {
            id: LinearIssueId::new("issue-alpha").unwrap(),
            identifier: "LOO-127".to_string(),
            title: "Repository-owned Waves".to_string(),
            description: "Preserve Task history through relocation.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: project.id.clone(),
        worktree: repo.join("task-worktree"),
        workspace_slug: "repository-owned-waves".to_string(),
        lifecycle: TaskLifecyclePlan::defaults(),
        lifecycle_phase: TaskLifecyclePhase::Loop,
        phase_epoch: 1,
        phase_cursor: 0,
        phase_iteration: 0,
        gate_cycle: 0,
        gate_proposal: None,
        agent: "codex".to_string(),
        provider: "codex".to_string(),
        provider_session_id: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: id,
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: "jack/repository-owned-waves".to_string(),
        base_commit: "deadbeef".to_string(),
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
    (task, pr)
}

fn lf_command(home: &Path, repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID")
        .env_remove("LF_ACCOUNT_LEASE")
        .output()
        .unwrap()
}

fn lf_output(home: &Path, repo: &Path, args: &[&str]) -> std::process::Output {
    let output = lf_command(home, repo, args);
    assert!(
        output.status.success(),
        "lf {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn lf(home: &Path, repo: &Path, args: &[&str]) -> serde_json::Value {
    let output = lf_output(home, repo, args);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn count_wave_runs(database: &Path, wave_id: &WaveId) -> i64 {
    rusqlite::Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT COUNT(*)
             FROM runs
             JOIN epochs ON epochs.id = runs.epoch_id
             LEFT JOIN projects ON projects.id = epochs.project_id
             LEFT JOIN tasks ON tasks.id = epochs.task_id
             LEFT JOIN projects task_projects ON task_projects.id = tasks.project_id
             WHERE epochs.wave_id = ?1
                OR projects.wave_id = ?1
                OR task_projects.wave_id = ?1",
            [wave_id.as_str()],
            |row| row.get(0),
        )
        .unwrap()
}

fn capture_project_invocation(
    database: &Path,
    run_id: &loopflow::durable::RunId,
    invocation_id: &loopflow::durable::AgentInvocationId,
) {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let invocation = AgentInvocationRow {
        id: invocation_id.to_string(),
        run_id: "trace-alpha".to_string(),
        answer_ask_id: None,
        process_id: "process-alpha".to_string(),
        started_at: now - 1,
        ended_at: Some(now),
        repo: "/historical/alpha".to_string(),
        worktree: "/historical/alpha".to_string(),
        wave: Some("infrastructure".to_string()),
        flow: Some("project".to_string()),
        skill: Some("repository-ownership".to_string()),
        project: Some("architecture".to_string()),
        task: None,
        provider: "codex".to_string(),
        model: Some("gpt-5".to_string()),
        surface: "headless".to_string(),
        capture_status: "complete".to_string(),
        incomplete_reason: None,
        outcome: "completed".to_string(),
        artifact_dir: "traces/repository-ownership".to_string(),
        conversation_path: "traces/repository-ownership/conversation.jsonl".to_string(),
        provider_events_path: None,
        provider_session_id: None,
        provider_session_path: None,
        conversation_event_count: 1,
        conversation_bytes: 1,
        supervision: Some(SupervisedInvocation {
            invocation_id: invocation_id.clone(),
            supervising_run_id: run_id.clone(),
            account_id: None,
            resume_token: None,
        }),
    };
    let turn = AgentTurnRow {
        id: "turn-repository-ownership".to_string(),
        invocation_id: invocation.id.clone(),
        ordinal: 1,
        provider_turn_id: None,
        started_at: now - 1,
        ended_at: Some(now),
        status: "completed".to_string(),
        input_op: "initial".to_string(),
        context_coverage: "assembled".to_string(),
        tokenizer: "o200k_base".to_string(),
        system_prompt_path: None,
        task_prompt_path: "traces/repository-ownership/task.md".to_string(),
        system_tokens: 0,
        task_tokens: 1,
        supplied_context_tokens: 1,
        usage: None,
        context_gather_ms: 1,
        context_render_ms: 1,
        context_persist_ms: 1,
        first_event_seq: None,
        last_event_seq: None,
        root_output: None,
        basis: None,
    };
    SqliteStore::new(database)
        .unwrap()
        .insert_trace_capture(&invocation, &turn, &[], &[])
        .unwrap();
}

#[tokio::test]
async fn repositories_own_same_named_waves_and_relocation_preserves_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let repo_a = tmp.path().join("alpha");
    let repo_b = tmp.path().join("beta");
    let repo_c = tmp.path().join("gamma");
    let repo_wrong_team = tmp.path().join("wrong-team");
    repository(&repo_a);
    repository(&repo_b);
    repository(&repo_c);
    repository(&repo_wrong_team);
    author_wave(&repo_a, "infrastructure", "alpha");
    author_wave(&repo_b, "infrastructure", "beta");

    std::fs::create_dir_all(&home).unwrap();
    let database = home.join("loopflow.db");
    let store = open_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let alpha = registered_wave(&repo_a, "infrastructure");
    let beta = registered_wave(&repo_b, "infrastructure");
    store.create_wave(&alpha).await.unwrap();
    store.create_wave(&beta).await.unwrap();
    assert_ne!(alpha.id(), beta.id());

    #[cfg(unix)]
    {
        let alias = tmp.path().join("alpha-alias");
        std::os::unix::fs::symlink(&repo_a, &alias).unwrap();
        let legacy = Wave::new(
            WaveId::new(),
            "legacy".to_string(),
            alias.display().to_string(),
        );
        store.create_wave(&legacy).await.unwrap();
        let resolved = store
            .get_wave_at(&WaveLocator::discover(&repo_a, "legacy").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resolved.id(), legacy.id());
        assert_eq!(
            resolved.repo(),
            std::fs::canonicalize(&repo_a)
                .unwrap()
                .display()
                .to_string()
        );
    }

    let alpha_resolved =
        resolve_managed_wave(Some(&store), Some(&repo_a), Some("infrastructure"), None)
            .await
            .unwrap();
    let beta_resolved =
        resolve_managed_wave(Some(&store), Some(&repo_b), Some("infrastructure"), None)
            .await
            .unwrap();
    assert_eq!(alpha_resolved.id(), alpha.id());
    assert_eq!(beta_resolved.id(), beta.id());
    assert!(matches!(
        resolve_managed_wave(Some(&store), None, Some("infrastructure"), None).await,
        Err(WaveResolveError::AmbiguousWave { .. })
    ));
    assert!(matches!(
        resolve_managed_wave(Some(&store), Some(&repo_b), None, Some(alpha.id().as_str()),).await,
        Err(WaveResolveError::RepositoryMismatch { .. })
    ));

    // `lf ls` is scoped to the invoking repository: from repo_a only alpha's
    // infrastructure Wave is listed, not repo_b's same-named beta.
    let scoped = lf(&home, &repo_a, &["ls", "--json"]);
    let scoped_infra: Vec<_> = scoped
        .as_array()
        .unwrap()
        .iter()
        .filter(|wave| wave["name"] == "infrastructure")
        .collect();
    assert_eq!(scoped_infra.len(), 1);
    assert_eq!(scoped_infra[0]["id"], alpha.id().as_str());

    // `--all` restores the machine-wide view: both same-named Waves appear.
    let all = lf(&home, &repo_a, &["ls", "--all", "--json"]);
    assert_eq!(
        all.as_array()
            .unwrap()
            .iter()
            .filter(|wave| wave["name"] == "infrastructure")
            .count(),
        2
    );
    assert_eq!(
        lf(&home, &repo_a, &["status", "infrastructure", "--json"])["wave"]["id"],
        alpha.id().as_str()
    );
    assert_eq!(
        lf(&home, &repo_b, &["status", "infrastructure", "--json"])["wave"]["id"],
        beta.id().as_str()
    );
    let human_list = String::from_utf8(lf_output(&home, &repo_a, &["ls"]).stdout).unwrap();
    assert!(human_list.contains("REPOSITORY"));
    assert!(human_list
        .lines()
        .any(|line| { line.contains("infrastructure") && line.contains("alpha") }));
    // repo_b's beta is out of scope from repo_a's checkout.
    assert!(!human_list
        .lines()
        .any(|line| { line.contains("infrastructure") && line.contains("beta") }));
    // `--all` brings beta's repository back into the human listing.
    let human_all = String::from_utf8(lf_output(&home, &repo_a, &["ls", "--all"]).stdout).unwrap();
    assert!(human_all
        .lines()
        .any(|line| { line.contains("infrastructure") && line.contains("beta") }));

    let alpha_placement = store
        .placement(&WorkRef::Wave(alpha.id().clone()))
        .await
        .unwrap();
    let foreign_home = HomeId::new();
    store
        .observe_home(&foreign_home, "ssh://operator@foreign-home")
        .await
        .unwrap();
    let foreign_place = lf_command(
        &home,
        &repo_b,
        &[
            "work",
            "place",
            "wave",
            alpha.id().as_str(),
            foreign_home.as_str(),
            "--json",
        ],
    );
    assert!(!foreign_place.status.success());
    assert!(String::from_utf8_lossy(&foreign_place.stderr).contains("not invoking repository"));
    assert_eq!(
        store
            .placement(&WorkRef::Wave(alpha.id().clone()))
            .await
            .unwrap()
            .home_id,
        alpha_placement.home_id
    );

    store
        .put_pm_snapshot(PmSnapshotRow {
            wave_id: alpha.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-alpha".to_string(),
            synced_at: 1,
            payload: r#"{"projects":[],"items":[]}"#.to_string(),
        })
        .await
        .unwrap();
    let project = project(&alpha);
    store.create_project(&project).await.unwrap();
    let (task, task_pr) = task(&alpha, &project, &repo_a);
    store.create_task(&task, &task_pr).await.unwrap();
    let placement = alpha_placement;
    let task_work = WorkRef::Task(task.id.clone());
    let (_, lease) = store
        .reserve_run(&task_work, RunTrigger::User)
        .await
        .unwrap();
    store
        .advance_run(
            &lease,
            RunAdvance::RunStarting {
                containment: Containment::ProcessGroup { id: 7 },
                cwd: repo_a.clone(),
            },
        )
        .await
        .unwrap();
    let invocation = store
        .advance_run(
            &lease,
            RunAdvance::InvocationStarting {
                route: InvocationRoute {
                    provider: "codex".to_string(),
                    model: Some("gpt-5".to_string()),
                    account_id: None,
                },
                surface: "headless".to_string(),
                resume_token: None,
                answer_ask_id: None,
            },
        )
        .await
        .unwrap();
    let AdvanceReceipt::Invocation(invocation) = invocation else {
        panic!("expected Invocation receipt")
    };
    capture_project_invocation(&database, &lease.run_id, &invocation.id);

    let active_error = relocate_wave(&store, alpha.id(), &repo_a, None, Some("platform"))
        .await
        .unwrap_err();
    assert!(active_error.to_string().contains("while Run"));
    assert_eq!(
        store.get_wave(alpha.id()).await.unwrap().unwrap().name(),
        "infrastructure"
    );
    assert!(repo_a.join("wave/infrastructure").is_dir());
    store
        .stop_run(&lease, StopCause::Requested, ContainmentObservation::Absent)
        .await
        .unwrap();
    let runs_before = count_wave_runs(&database, alpha.id());

    let overlap = relocate_wave(
        &store,
        alpha.id(),
        &repo_a,
        None,
        Some("infrastructure/platform"),
    )
    .await
    .unwrap_err();
    assert!(overlap.to_string().contains("paths overlap"));
    assert!(repo_a.join("wave/infrastructure/GOAL.md").is_file());

    for (repo, team) in [
        (&repo_a, "team-alpha"),
        (&repo_c, "team-alpha"),
        (&repo_wrong_team, "team-beta"),
    ] {
        std::fs::create_dir_all(repo.join(".lf")).unwrap();
        std::fs::write(
            repo.join(".lf/config.yaml"),
            format!("pm:\n  linear_team: {team}\n"),
        )
        .unwrap();
    }
    let team_error = relocate_wave(&store, alpha.id(), &repo_a, Some(&repo_wrong_team), None)
        .await
        .unwrap_err();
    assert!(team_error.to_string().contains("repository Team"));

    author_wave(&repo_a, "infrastructure/child", "nested");
    let nested = registered_wave(&repo_a, "infrastructure/child");
    store.create_wave(&nested).await.unwrap();
    let nested_error = relocate_wave(&store, alpha.id(), &repo_a, None, Some("platform"))
        .await
        .unwrap_err();
    assert!(nested_error
        .to_string()
        .contains("contains registered Wave"));
    assert!(repo_a.join("wave/infrastructure/child/GOAL.md").is_file());
    store.delete_wave(nested.id()).await.unwrap();
    std::fs::remove_dir_all(repo_a.join("wave/infrastructure/child")).unwrap();

    author_wave(&repo_a, "infrastructure/child", "chord-child");
    let child = registered_wave(&repo_a, "infrastructure/child").with_parent(alpha.id().clone());
    store.create_wave(&child).await.unwrap();

    let occupied = registered_wave(&repo_a, "occupied");
    store.create_wave(&occupied).await.unwrap();
    let collision = relocate_wave(&store, alpha.id(), &repo_a, None, Some("occupied"))
        .await
        .unwrap_err();
    assert!(collision.to_string().contains("already belongs"));

    author_wave(&repo_a, "platform", "divergent");
    let divergence = relocate_wave(&store, alpha.id(), &repo_a, None, Some("platform"))
        .await
        .unwrap_err();
    assert!(divergence.to_string().contains("diverges"));
    assert!(repo_a.join("wave/infrastructure").is_dir());
    std::fs::remove_dir_all(repo_a.join("wave/platform")).unwrap();
    std::fs::remove_dir_all(repo_a.join(".lf/journal/waves/platform")).unwrap();

    rusqlite::Connection::open(&database)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_wave_relocation
             BEFORE UPDATE OF repo, name ON waves
             BEGIN SELECT RAISE(ABORT, 'injected relocation failure'); END;",
        )
        .unwrap();
    let injected = relocate_wave(&store, alpha.id(), &repo_a, None, Some("platform"))
        .await
        .unwrap_err();
    assert!(injected.to_string().contains("injected relocation failure"));
    assert_eq!(
        store.get_wave(alpha.id()).await.unwrap().unwrap().name(),
        "infrastructure"
    );
    assert!(repo_a.join("wave/infrastructure").is_dir());
    assert!(repo_a.join("wave/platform").is_dir());
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute_batch("DROP TRIGGER fail_wave_relocation")
        .unwrap();

    let relocation = lf(
        &home,
        &repo_a,
        &[
            "work",
            "relocate",
            "wave",
            alpha.id().as_str(),
            "--name",
            "platform",
            "--json",
        ],
    );
    assert_eq!(relocation["kind"], "relocated");
    assert_eq!(relocation["wave_id"], alpha.id().as_str());
    assert_eq!(relocation["waves_moved"], 2);
    let renamed = store.get_wave(alpha.id()).await.unwrap().unwrap();
    assert_eq!(renamed.name(), "platform");
    assert_eq!(
        std::fs::read_to_string(repo_a.join("wave/platform/GOAL.md")).unwrap(),
        "# alpha\n"
    );
    assert!(!repo_a.join("wave/infrastructure").exists());
    let renamed_child = store.get_wave(child.id()).await.unwrap().unwrap();
    assert_eq!(renamed_child.name(), "platform/child");
    assert!(repo_a.join("wave/platform/child/GOAL.md").is_file());
    assert_eq!(
        store
            .get_wave_at(&WaveLocator::discover(&repo_b, "infrastructure").unwrap())
            .await
            .unwrap()
            .unwrap()
            .id(),
        beta.id()
    );

    relocate_wave(&store, alpha.id(), &repo_a, Some(&repo_c), None)
        .await
        .unwrap();
    assert!(!repo_a.join("wave/platform").exists());
    assert_eq!(
        std::fs::read_to_string(repo_c.join("wave/platform/GOAL.md")).unwrap(),
        "# alpha\n"
    );

    let repo_d = tmp.path().join("delta");
    std::fs::rename(&repo_c, &repo_d).unwrap();
    relocate_wave(&store, alpha.id(), &repo_d, Some(&repo_d), None)
        .await
        .unwrap();
    let moved = store.get_wave(alpha.id()).await.unwrap().unwrap();
    assert_eq!(
        moved.repo(),
        std::fs::canonicalize(&repo_d)
            .unwrap()
            .display()
            .to_string()
    );
    assert_eq!(moved.name(), "platform");
    let moved_child = store.get_wave(child.id()).await.unwrap().unwrap();
    assert_eq!(moved_child.repo(), moved.repo());
    assert_eq!(moved_child.name(), "platform/child");
    assert_eq!(
        store
            .pm_snapshot(alpha.id())
            .await
            .unwrap()
            .unwrap()
            .initiative,
        "initiative-alpha"
    );
    assert_eq!(
        store
            .placement(&WorkRef::Wave(alpha.id().clone()))
            .await
            .unwrap()
            .home_id,
        placement.home_id
    );
    assert_eq!(count_wave_runs(&database, alpha.id()), runs_before);
    assert_eq!(
        store
            .get_project(&project.id)
            .await
            .unwrap()
            .unwrap()
            .wave_id,
        alpha.id().clone()
    );
    let preserved_task = store.get_task(&task.id).await.unwrap().unwrap();
    assert_eq!(preserved_task.wave_id, alpha.id().clone());
    assert_eq!(preserved_task.project_id, project.id);
    assert!(journal::journal_path(&repo_d, "platform").is_file());

    let status = lf(&home, &repo_d, &["status", "platform", "--json"]);
    assert_eq!(status["wave"]["id"], alpha.id().as_str());
    assert!(status["runs"]["items"].as_array().is_some_and(|items| items
        .iter()
        .any(|item| { item.to_string().contains("repository-ownership") })));

    let error = relocate_wave(&store, alpha.id(), &repo_b, None, Some("hijacked"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("invoke relocation from"));
    assert_eq!(
        store.get_wave(alpha.id()).await.unwrap().unwrap().name(),
        "platform"
    );
    assert_eq!(
        store
            .get_wave_at(&WaveLocator::discover(&repo_b, "infrastructure").unwrap())
            .await
            .unwrap()
            .unwrap()
            .id(),
        beta.id()
    );
}
