use std::path::Path;
use std::process::Command;

use loopflow::durable::{HomeId, WorkRef, WorkStatus};
use loopflow::engine::wave_context::{resolve_managed_wave, WaveResolveError};
use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::project::{Project, ProjectId};
use loopflow::store::{open_store, PmSnapshotRow, StorageConfig};
use loopflow::task::{
    Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
    TaskPrId,
};
use loopflow::wave::journal;
use loopflow::wave::relocate::relocate_wave;
use loopflow::wave::{Wave, WaveLocator};
use time::OffsetDateTime;

fn repository(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["config", "user.email", "test@example.com"]);
    git(path, &["config", "user.name", "Test User"]);
    std::fs::write(path.join(".gitignore"), ".lf/\n").unwrap();
    commit(path, "initial");
}

fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit(repo: &Path, message: &str) {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", message]);
}

fn author_wave(repo: &Path, slug: &str, marker: &str) {
    let wave = repo.join("wave").join(slug);
    std::fs::create_dir_all(&wave).unwrap();
    std::fs::write(wave.join("GOAL.md"), format!("# {marker}\n")).unwrap();
    let journal = journal::journal_path(repo, slug);
    std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
    std::fs::write(journal, format!("{{\"repository\":\"{marker}\"}}\n")).unwrap();
    commit(repo, &format!("author {slug}"));
}

fn registered_wave(repo: &Path, slug: &str) -> Wave {
    let locator = WaveLocator::discover(repo, slug).unwrap();
    Wave::new(
        WaveId::new(),
        locator.slug().to_string(),
        locator.repo().to_string(),
    )
}

fn apply_status_truth(database: &Path) {
    let connection = rusqlite::Connection::open(database).unwrap();
    let has_retirement = connection
        .prepare("PRAGMA table_info(waves)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .any(|column| column.is_ok_and(|column| column == "retired_at"));
    if has_retirement {
        return;
    }
    connection
        .execute_batch(&loopflow_test_support::migration_sql_for_test(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "status_truth",
        ))
        .unwrap();
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
        .env_remove("LF_RUN_ID")
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
    apply_status_truth(&database);
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
    store
        .put_pm_snapshot(PmSnapshotRow {
            wave_id: occupied.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-occupied".to_string(),
            synced_at: 1,
            payload: r#"{"projects":[],"items":[]}"#.to_string(),
        })
        .await
        .unwrap();
    let collision = relocate_wave(&store, alpha.id(), &repo_a, None, Some("occupied"))
        .await
        .unwrap_err();
    assert!(collision.to_string().contains(alpha.id().as_str()));
    assert!(collision.to_string().contains(occupied.id().as_str()));
    assert!(collision.to_string().contains("PM snapshot"));

    author_wave(&repo_a, "platform", "divergent");
    let divergence = relocate_wave(&store, alpha.id(), &repo_a, None, Some("platform"))
        .await
        .unwrap_err();
    assert!(divergence.to_string().contains("diverges"));
    assert!(repo_a.join("wave/infrastructure").is_dir());
    std::fs::remove_dir_all(repo_a.join("wave/platform")).unwrap();
    std::fs::remove_dir_all(repo_a.join(".lf/journal/waves/platform")).unwrap();
    commit(&repo_a, "remove divergent target");

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
    commit(&repo_a, "record staged relocation");
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
    commit(&repo_a, "record Wave rename");

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

#[tokio::test]
async fn missing_repository_wave_can_be_disabled_and_relocated_from_its_target() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let source = tmp.path().join("missing-source");
    let target = tmp.path().join("surviving-target");
    repository(&source);
    repository(&target);
    author_wave(&source, "feedback", "feedback");
    author_wave(&target, "feedback", "feedback");
    std::fs::create_dir_all(&home).unwrap();
    let database = home.join("loopflow.db");
    let store = open_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    apply_status_truth(&database);
    let wave = registered_wave(&source, "feedback");
    store.create_wave(&wave).await.unwrap();

    std::fs::remove_dir_all(&source).unwrap();
    let disabled = lf(
        &home,
        &target,
        &["work", "disable", "wave", wave.id().as_str(), "--json"],
    );
    assert_eq!(disabled["kind"], "disabled");
    assert!(!disabled["enabled"].as_bool().unwrap());

    let relocated = lf(
        &home,
        &target,
        &[
            "work",
            "relocate",
            "wave",
            wave.id().as_str(),
            "--repo",
            target.to_str().unwrap(),
            "--json",
        ],
    );
    assert_eq!(relocated["wave_id"], wave.id().as_str());
    let repaired = store.get_wave(wave.id()).await.unwrap().unwrap();
    assert_eq!(
        repaired.repo(),
        std::fs::canonicalize(&target)
            .unwrap()
            .display()
            .to_string()
    );
    assert!(
        !store
            .placement(&WorkRef::Wave(wave.id().clone()))
            .await
            .unwrap()
            .enabled
    );
}

#[tokio::test]
async fn relocation_retires_an_empty_destination_shadow_without_losing_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("cadenza");
    let target = tmp.path().join("kata");
    repository(&source);
    repository(&target);
    for slug in ["core", "ear", "theory"] {
        author_wave(&source, slug, slug);
        author_wave(&target, slug, slug);
    }
    author_wave(&target, "scores", "scores");
    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let database = home.join("loopflow.db");
    let store = open_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    apply_status_truth(&database);
    let identities = ["core", "ear", "theory"].map(|slug| {
        (
            registered_wave(&source, slug),
            registered_wave(&target, slug),
        )
    });
    let scores = registered_wave(&target, "scores");
    for (established, shadow) in &identities {
        store.create_wave(established).await.unwrap();
        store.create_wave(shadow).await.unwrap();
    }
    store.create_wave(&scores).await.unwrap();
    let initial_wave_count: i64 = rusqlite::Connection::open(&database)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM waves", [], |row| row.get(0))
        .unwrap();

    for (established, shadow) in &identities {
        let receipt = relocate_wave(&store, established.id(), &source, Some(&target), None)
            .await
            .unwrap();
        assert_eq!(receipt.wave_id, established.id().as_str());
        commit(&source, &format!("relocate {}", established.name()));

        let active = store
            .get_wave_at(&WaveLocator::discover(&target, established.name()).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active.id(), established.id());
        let retired = store.get_wave(shadow.id()).await.unwrap().unwrap();
        assert!(retired.is_retired());
        assert_eq!(retired.superseded_by_wave_id(), Some(established.id()));
        assert!(retired
            .retirement_reason()
            .unwrap()
            .contains("registration-only"));
        assert_eq!(
            store
                .work_status(&WorkRef::Wave(shadow.id().clone()))
                .await
                .unwrap(),
            WorkStatus::Abandoned
        );
        assert!(
            !store
                .placement(&WorkRef::Wave(shadow.id().clone()))
                .await
                .unwrap()
                .enabled
        );

        let historical = resolve_managed_wave(
            Some(&store),
            Some(&target),
            Some(shadow.id().as_str()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(historical.id(), shadow.id());
        assert!(historical.is_retired());
        let historical_status =
            String::from_utf8(lf_output(&home, &target, &["status", shadow.id().as_str()]).stdout)
                .unwrap();
        assert!(historical_status.contains("retired at"));
        assert!(historical_status.contains(established.id().as_str()));
    }
    assert_eq!(
        rusqlite::Connection::open(&database)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM waves", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        initial_wave_count,
        "relocation must not mint another Wave identity"
    );
    assert_eq!(
        store
            .get_wave_at(&WaveLocator::discover(&target, "scores").unwrap())
            .await
            .unwrap()
            .unwrap()
            .id(),
        scores.id()
    );
    assert_eq!(store.list_waves(None).await.unwrap().len(), 4);

    let reopened = open_store(&StorageConfig::sqlite(database)).await.unwrap();
    assert_eq!(reopened.list_waves(None).await.unwrap().len(), 4);
    for (established, _) in &identities {
        assert_eq!(
            reopened
                .get_wave_at(&WaveLocator::discover(&target, established.name()).unwrap())
                .await
                .unwrap()
                .unwrap()
                .id(),
            established.id()
        );
    }
}

#[tokio::test]
async fn relocation_refuses_meaningful_destination_history() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source");
    let target = tmp.path().join("target");
    repository(&source);
    repository(&target);
    author_wave(&source, "core", "core");
    let database = tmp.path().join("loopflow.db");
    let store = open_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    apply_status_truth(&database);
    let established = registered_wave(&source, "core");
    store.create_wave(&established).await.unwrap();

    let project_shadow = registered_wave(&target, "with-task");
    store.create_wave(&project_shadow).await.unwrap();
    let owned_project = project(&project_shadow);
    store.create_project(&owned_project).await.unwrap();
    let (owned_task, owned_pr) = task(&project_shadow, &owned_project, &target);
    store.create_task(&owned_task, &owned_pr).await.unwrap();

    let child_shadow = registered_wave(&target, "with-child");
    store.create_wave(&child_shadow).await.unwrap();
    let child =
        registered_wave(&target, "with-child/nested").with_parent(child_shadow.id().clone());
    store.create_wave(&child).await.unwrap();

    let pm_shadow = registered_wave(&target, "with-pm");
    store.create_wave(&pm_shadow).await.unwrap();
    store
        .put_pm_snapshot(PmSnapshotRow {
            wave_id: pm_shadow.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-pm".to_string(),
            synced_at: 1,
            payload: r#"{"projects":[],"items":[]}"#.to_string(),
        })
        .await
        .unwrap();

    let receipt_shadow = registered_wave(&target, "with-receipt");
    store.create_wave(&receipt_shadow).await.unwrap();
    let receipt_path = target
        .join(".lf/tmp/wave-relocations")
        .join(format!("{}.json", receipt_shadow.id()));
    std::fs::create_dir_all(receipt_path.parent().unwrap()).unwrap();
    std::fs::write(&receipt_path, "{}\n").unwrap();

    for (slug, shadow, evidence) in [
        ("with-task", &project_shadow, "Tasks"),
        ("with-child", &child_shadow, "child Waves"),
        ("with-pm", &pm_shadow, "PM snapshot"),
        ("with-receipt", &receipt_shadow, "relocation receipt"),
    ] {
        let error = relocate_wave(&store, established.id(), &source, Some(&target), Some(slug))
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(established.id().as_str()), "{error}");
        assert!(error.contains(shadow.id().as_str()), "{error}");
        assert!(error.contains(evidence), "{error}");
    }

    assert_eq!(
        store
            .get_wave(established.id())
            .await
            .unwrap()
            .unwrap()
            .repo(),
        established.repo()
    );
}
