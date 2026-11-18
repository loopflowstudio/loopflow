use super::{apply_before_migration, apply_sqlite, upgrade_from_migration};
use rusqlite::OptionalExtension;

#[test]
fn settings_authority_preserves_task_history_and_migrates_preferences() {
    let home = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    apply_before_migration(&conn, "settings_authority");
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../loopflow-storage/tests/fixtures/migrations/settings_authority_released.sql"
    )))
    .unwrap();
    let mut guards = Vec::new();
    crate::config::migrate::preflight(&conn, &mut guards).unwrap();
    drop(guards);
    upgrade_from_migration(&conn, "settings_authority");
    let retained: (String, String) = conn.query_row("SELECT c.worktree,s.provider FROM tasks t JOIN task_checkouts c ON c.task_id=t.id JOIN agent_sessions s ON s.task_id=t.id WHERE t.id='retained'", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(retained, ("/repo.task".into(), "claude".into()));
    let values: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&std::fs::read_to_string(home.path().join("config.yaml")).unwrap())
            .unwrap();
    assert_eq!(values["skills"]["design"]["agent"], "codex:o3");
    assert_eq!(values["skills"]["implement"]["agent"], "codex");
    assert_eq!(conn.query_row("SELECT count(*) FROM sqlite_master WHERE name IN ('scope_settings','scope_defaults','provider_routes')", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
}

#[tokio::test]
async fn retired_home_landings_remain_readable_and_fence_old_supervisors() {
    use std::sync::Arc;

    use crate::ops::pr_landing::{reconcile_pr_landing, LandingIo, LandingObservation, ObservePr};
    use loopflow_storage::pr_landing::{
        LandingPlacement, LandingSupervisor, PrLandingId, PrLandingState,
    };

    use time::OffsetDateTime;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("registry.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    apply_before_migration(&conn, "retire_home_landing_supervisors");
    conn.execute_batch(
        "INSERT INTO homes (id, route, created_at, observed_at)
         VALUES ('retired_home', 'remote', 1, 2);
         INSERT INTO waves (id, name, repo, created_at)
         VALUES ('landing_wave', 'landing', '/repo', 1);
         INSERT INTO projects (id, wave_id, external_project_id, created_at, updated_at)
         VALUES ('landing_project', 'landing_wave', 'linear-project', 1, 1);
         INSERT INTO tasks (id, project_id, external_issue_id, issue_identifier,
            worktree, workspace_slug, created_at, updated_at)
         VALUES ('landing_task', 'landing_project', 'linear-task', 'INF-1',
            '/tmp/landing', 'landing', 1, 1);",
    )
    .unwrap();
    for (index, state) in ["watching", "repairing", "blocked", "merged", "closed"]
        .iter()
        .enumerate()
    {
        for placement in ["home", "local"] {
            conn.execute(
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../loopflow-storage/tests/fixtures/migrations/retired_home_landing.sql"
                )),
                rusqlite::params![
                    format!("{placement}_{state}"),
                    index as i64 + if placement == "home" { 1 } else { 11 },
                    (*state == "merged").then_some("merge"),
                    state,
                    placement,
                    (placement == "home").then_some("retired_home"),
                    (*state == "blocked").then_some("credential revoked"),
                ],
            )
            .unwrap();
        }
    }
    // A taskless retained delivery can settle through the real reconciler.
    assert!(std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(directory.path())
        .status()
        .unwrap()
        .success());
    conn.execute(
        "INSERT INTO pr_landings (id, repo, pr_number, worktree, branch,
            requested_head_sha, observed_head_sha, state, generation,
            supervisor_placement, supervisor_home_id, supervisor_process_id,
            supervisor_heartbeat_at, created_at, updated_at)
         VALUES ('home_taskless', 'taskless/repo', 42, ?1, 'repair', 'head', 'head',
            'watching', 7, 'home', 'retired_home', 123, 30, 10, 40)",
        [directory.path().to_str().unwrap()],
    )
    .unwrap();
    apply_sqlite(&conn).unwrap();
    drop(conn);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    assert_eq!(store.pending_pr_landings("owner/repo").unwrap().len(), 6);
    let now = OffsetDateTime::from_unix_timestamp(50).unwrap();
    for state in ["watching", "repairing", "blocked", "merged", "closed"] {
        for placement in ["home", "local"] {
            let id = PrLandingId::from_raw(format!("{placement}_{state}"));
            let landing = store.get_pr_landing(&id).unwrap().unwrap();
            assert_eq!(landing.state.as_str(), state);
            assert_eq!(landing.requested_head_sha, "requested");
            assert_eq!(landing.observed_head_sha, "observed");
            assert_eq!(landing.task_id.as_ref().unwrap().as_str(), "landing_task");
            let historical: (String, String) = rusqlite::Connection::open(&path)
                .unwrap()
                .query_row(
                    "SELECT after_merge,next_slug FROM pr_landings WHERE id=?1",
                    [id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(historical, ("continue_task".into(), "follow-up".into()));
            assert_eq!(landing.updated_at.unix_timestamp(), 40);
            assert_eq!(
                landing.blocked_reason.as_deref(),
                (state == "blocked").then_some("credential revoked")
            );
            if placement == "local" {
                assert_eq!(landing.generation, 7);
                assert_eq!(landing.supervisor.unwrap().process_id, 123);
                continue;
            }
            assert_eq!(landing.generation, 8);
            assert!(landing.supervisor.is_none());
            assert!(!store.heartbeat_pr_landing(&id, 7, now).unwrap());
            let mut stale = landing;
            stale.generation = 7;
            stale.state = PrLandingState::Closed;
            stale.blocked_reason = None;
            assert!(!store.update_pr_landing(&stale).unwrap());
            assert!(store
                .claim_pr_landing(
                    &id,
                    7,
                    &LandingSupervisor {
                        placement: LandingPlacement::Local,
                        process_id: 456,
                        heartbeat_at: now,
                    },
                    now
                )
                .unwrap()
                .is_none());
        }
    }
    drop(store);
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    assert_eq!(store.pending_pr_landings("owner/repo").unwrap().len(), 6);
    let conn = rusqlite::Connection::open(&path).unwrap();
    assert!(conn.execute(
        "UPDATE pr_landings SET supervisor_placement='home', supervisor_machine_id='retired_home',
            supervisor_process_id=456, supervisor_heartbeat_at=50 WHERE id='home_watching' AND generation=8", [],
    ).is_err());
    let history: String = conn
        .query_row(
            "SELECT retired_supervisor_json FROM pr_landings WHERE id='home_blocked'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&history).unwrap(),
        serde_json::json!({
            "placement": "home", "home_id": "retired_home", "process_id": 123,
            "heartbeat_at": 30, "generation": 7,
        })
    );
    assert!(conn
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()
        .unwrap()
        .is_none());
    drop(conn);
    drop(store);

    // A merged delivery is observed without ever being offered a repair.
    let merged_observation: ObservePr = Arc::new(|_| {
        Ok(LandingObservation::Merged {
            head_sha: "head".into(),
            merge_commit: "merge".into(),
        })
    });
    let store = Arc::new(
        crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(path))
            .await
            .unwrap(),
    );
    let id = PrLandingId::from_raw("home_taskless");
    let landing = store.get_pr_landing(&id).await.unwrap().unwrap();
    let merged = reconcile_pr_landing(
        store.clone(),
        landing,
        LandingIo::scripted(merged_observation.clone(), None),
    )
    .await
    .unwrap();
    assert_eq!(merged.state, PrLandingState::Merged);
    assert_eq!(merged.merge_commit.as_deref(), Some("merge"));
    let repeated = reconcile_pr_landing(
        store.clone(),
        merged.clone(),
        LandingIo::scripted(merged_observation, None),
    )
    .await
    .unwrap();
    assert_eq!(repeated, merged);
    assert_eq!(
        store.pending_pr_landings("owner/repo").await.unwrap().len(),
        6
    );
}
