//! Apply Linear events within the originating request.

use crate::store::{run_planning_write, PlanningLocks, Store, StoreResult};
use loopflow_model::id::WaveId;
use std::sync::Arc;

use loopflow_storage::sqlite::linear_events::{
    accept_linear_inventory_in, accept_linear_issue_in, reconcile_linear_project_teams_in,
};

pub(crate) async fn accept_linear_inventory(
    store: &Store,
    wave: WaveId,
    initiative: String,
    projects: Vec<loopflow_storage::planning::TrackerProject>,
    issues: Vec<loopflow_storage::planning::TrackerItem>,
    observed_at: i64,
    locks: Option<Arc<PlanningLocks>>,
) -> StoreResult<()> {
    run_planning_write(&store.sqlite, locks, move |store| {
        accept_linear_inventory_in(&store, &wave, &initiative, &projects, &issues, observed_at)
    })
    .await
}

pub(crate) async fn reconcile_linear_project_teams(
    store: &Store,
    wave: &WaveId,
    initiative: &str,
    project: loopflow_storage::planning::TrackerProject,
    observed_at: i64,
    locks: Arc<PlanningLocks>,
) -> StoreResult<()> {
    let wave = wave.clone();
    let initiative = initiative.to_string();
    run_planning_write(&store.sqlite, Some(locks), move |store| {
        reconcile_linear_project_teams_in(&store, &wave, &initiative, &project, observed_at)
    })
    .await
}

pub(crate) async fn accept_linear_issue(
    store: &Store,
    repo: &str,
    issue: loopflow_storage::planning::TrackerItem,
    project: Option<loopflow_storage::planning::TrackerProject>,
    observed_at: i64,
    confirmed_wave: Option<(WaveId, String)>,
    locks: Option<Arc<PlanningLocks>>,
) -> StoreResult<()> {
    let repo = repo.to_string();
    run_planning_write(&store.sqlite, locks, move |store| {
        if store.repository_id(&repo)?.is_none() {
            crate::repository::location::ensure_repository(&store, &repo)?;
        }
        accept_linear_issue_in(
            &store,
            &repo,
            &issue,
            project.as_ref(),
            observed_at,
            confirmed_wave
                .as_ref()
                .map(|(wave, initiative)| (wave, initiative.as_str())),
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use crate::test_linear_input::SeedLinearSqlite;
    use rusqlite::Connection;

    use crate::store::PlanningState;
    use loopflow_model::id::WaveId;
    use loopflow_storage::sqlite::SqliteStore;

    #[test]
    fn explicit_null_detaches_but_unknown_linear_membership_retains_task() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let mut issue = snapshot.items[0].clone();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                initiative: snapshot.projects[0].initiative_ids[0].clone(),
                synced_at: 42,
                snapshot,
            })
            .unwrap();
        let original = store.task_by_issue(&issue.id).unwrap().unwrap();
        // Unmapped Project evidence cannot manufacture a standalone Task.
        issue.project_id = Some("unknown-project".into());
        issue.revision = Some("2026-10-17T01:00:00Z".into());
        loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
            &store,
            "/repo",
            &issue,
            None,
            43,
            None,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
        assert_eq!(
            store.task(&original.id).unwrap().unwrap().project_id,
            original.project_id
        );
        assert_eq!(
            store.task_tracker_state(&original.id).unwrap().state,
            PlanningState::Invalid
        );
        // An omitted field in retained historical evidence remains unknown.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tracker_items SET project_id=NULL,claimed_project_id=NULL,project_id_observed=0",
                [],
            )
            .unwrap();
        assert_eq!(
            store.linear_issue_state("/repo", &issue.id).unwrap().state,
            PlanningState::Invalid
        );
        assert_eq!(
            store.task(&original.id).unwrap().unwrap().project_id,
            original.project_id
        );
        // A complete, newer observation explicitly clearing Project detaches it.
        issue.project_id = None;
        issue.revision = Some("2026-10-17T02:00:00Z".into());
        loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
            &store,
            "/repo",
            &issue,
            None,
            44,
            None,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
        let detached = store.task(&original.id).unwrap().unwrap();
        assert!(detached.project_id.is_none());
        assert!(detached.wave_id.is_none());
        assert_eq!(detached.repository_id, original.repository_id);
        assert_eq!(
            store.task_tracker_state(&original.id).unwrap().state,
            PlanningState::Available
        );
    }

    #[test]
    fn task_admission_reads_released_evidence_without_reconstructing_tracker_content() {
        use crate::store::migrations::{apply_before_migration, upgrade_from_migration};
        use loopflow_storage::work::project::ProjectId;
        use loopflow_storage::work::task::TaskId;

        let cases = [
            ("", PlanningState::Available),
            ("UPDATE pm_items SET needs_refresh=1 WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_projects SET membership_unresolved=1 WHERE id='project'", PlanningState::Invalid),
            ("UPDATE pm_projects SET archived=1 WHERE id='project'", PlanningState::Invalid),
            ("UPDATE pm_items SET project_id='unmapped' WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_set(body,'$.project_id','unmapped') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_set(body,'$.team_id','foreign') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_remove(body,'$.team_id') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_remove(body,'$.project_id') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_projects SET body=json_set(body,'$.team_ids',json('[\"foreign\"]')) WHERE id='project'", PlanningState::Invalid),
            ("INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',1)", PlanningState::Removed),
            ("DELETE FROM pm_items WHERE id='issue'", PlanningState::Unavailable),
            ("DELETE FROM pm_items WHERE id='issue'; UPDATE pm_projects SET membership_unresolved=1 WHERE id='project'", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; UPDATE pm_projects SET archived=1 WHERE id='project'", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',0)", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',1)", PlanningState::Removed),
        ];
        let released = Connection::open_in_memory().unwrap();
        apply_before_migration(&released, "planning_peers");
        for (sql, expected) in cases {
            let mut conn = Connection::open_in_memory().unwrap();
            // Each case upgrades its own released data; the empty historical
            // schema is identical and need only be constructed once.
            rusqlite::backup::Backup::new(&released, &mut conn)
                .unwrap()
                .run_to_completion(256, std::time::Duration::ZERO, None)
                .unwrap();
            let wave = WaveId::new();
            let project = ProjectId::new();
            let task = TaskId::new();
            conn.execute_batch(
                &include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../loopflow-storage/tests/fixtures/migrations/planning_task_evidence.sql"
                ))
                .replace("{wave}", wave.as_str())
                .replace("{project}", project.as_str())
                .replace("{task}", task.as_str()),
            )
            .unwrap();
            conn.execute_batch(sql).unwrap();
            let retained: Vec<(String, String, i64)> = conn
                .prepare("SELECT 'issue:'||id,json_array(json_extract(body,'$.team_id'),json_extract(body,'$.project_id'),needs_refresh),observed_at FROM pm_items UNION ALL SELECT 'project:'||id,json_array(json_extract(body,'$.team_ids'),archived,membership_unresolved),observed_at FROM pm_projects ORDER BY 1")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            upgrade_from_migration(&conn, "planning_peers");
            let store =
                SqliteStore::from_fixture_connection(conn, crate::store::token_key::encryption_key);
            let saved = store.task(&task).unwrap().unwrap();
            assert_eq!(saved.title, "Saved Task");
            assert_eq!(saved.description, "Saved brief");
            assert_eq!(
                store.project(&project).unwrap().unwrap().description,
                "Preserved prose"
            );
            assert_eq!(
                saved.worktree.as_deref(),
                Some(std::path::Path::new("/retained/checkout"))
            );
            assert!(store.task_by_issue("OTHER-2").unwrap().is_none());
            let state = store.task_tracker_state(&task).unwrap();
            assert_eq!(state.state, expected, "{sql}");
            assert_eq!(
                state.synced_at,
                (!sql.contains("DELETE FROM pm_items")).then_some(42),
                "{sql}"
            );
            assert_eq!(
                store.validate_task_planning(&saved).is_err(),
                matches!(expected, PlanningState::Invalid | PlanningState::Removed),
                "{sql}"
            );
            assert_eq!(store.task(&task).unwrap().unwrap(), saved);
            let conn = store.conn.lock().unwrap();
            let after: Vec<(String, String, i64)> = conn
                .prepare("SELECT 'issue:'||id,json_array(team_id,claimed_project_id,needs_refresh),observed_at FROM tracker_items UNION ALL SELECT 'project:'||id,json_array(json(team_ids),archived,membership_unresolved),observed_at FROM tracker_projects ORDER BY 1")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(after, retained);
            drop(conn);
        }
    }

    #[test]
    fn ownership_reads_retained_claims_without_changing_work_or_observation_age() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let project = snapshot.projects[0].clone();
        let item = snapshot.items[0].clone();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                initiative: project.initiative_ids[0].clone(),
                synced_at: 42,
                snapshot,
            })
            .unwrap();
        let task = store.task_by_issue(&item.id).unwrap().unwrap();
        let validate = || {
            store.validate_linear_ownership(
                &[(wave.id(), wave.slug())],
                Ok(Some(project.team_ids[0].clone())),
            )
        };
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            // Foreign evidence blocks its Task, not the repository inventory.
            conn.execute("UPDATE tracker_items SET team_id='foreign'", [])
                .unwrap();
        }
        validate().unwrap();
        assert!(store.validate_task_planning(&task).is_err());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tracker_projects SET team_ids='[\"foreign\"]'", [])
                .unwrap();
        }
        validate().unwrap();
        assert!(store.validate_task_planning(&task).is_err());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tracker_items SET needs_refresh=1", [])
                .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tracker_items SET needs_refresh=0", [])
                .unwrap();
            conn.execute("UPDATE tracker_projects SET membership_unresolved=1", [])
                .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE tracker_projects SET membership_unresolved=0,archived=1",
                [],
            )
            .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE tracker_projects SET archived=0,initiative_ids='[\"removed\"]'",
                [],
            )
            .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE tracker_projects SET initiative_ids=?1",
                [serde_json::to_string(&project.initiative_ids).unwrap()],
            )
            .unwrap();
        }
        validate().unwrap();
        assert_eq!(store.task(&task.id).unwrap().unwrap(), task);
        let conn = store.conn.lock().unwrap();
        let ages: (i64, i64) = conn.query_row(
            "SELECT p.observed_at,i.observed_at FROM tracker_projects p JOIN tracker_items i ON i.project_id=p.id",
            [], |row| Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        assert_eq!(ages, (42, 42));
    }

    #[test]
    fn owned_issue_import_retains_identity_across_alias_changes_without_placement() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let initiative = snapshot.projects[0].initiative_ids[0].clone();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let mut row = crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(),
            initiative,
            synced_at: 42,
            snapshot,
        };
        store.seed_inventory(&row).unwrap();
        let item = &row.snapshot.items[0];
        let task = store.task_by_issue(&item.id).unwrap().unwrap();
        assert!(task.worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert_eq!(task.title, item.name);
        assert_eq!(task.tracker_synced_at, Some(42));
        store.seed_inventory(&row).unwrap();
        assert_eq!(store.task_by_issue(&item.id).unwrap().unwrap().id, task.id);
        row.synced_at = 43;
        row.snapshot.items[0].identifier = "MOVED-42".into();
        row.snapshot.items[0].revision = Some("2099-01-01T00:00:00Z".into());
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_by_issue("MOVED-42").unwrap().unwrap().id,
            task.id
        );
        // A detail event refreshes content, never the order observed by a list.
        let mut detail = row.snapshot.items[0].clone();
        detail.name = "Updated on another machine".into();
        detail.rank = 99;
        detail.revision = Some("2099-01-02T00:00:00Z".into());
        store
            .seed_issue(
                "/repo",
                "linear",
                &crate::test_linear_input::IssueFixture {
                    item: detail.clone(),
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 44,
                },
                Some((wave.id(), &row.initiative)),
            )
            .unwrap();
        let refreshed = store.task(&task.id).unwrap().unwrap();
        assert_eq!(refreshed.title, detail.name);
        assert_eq!(refreshed.tracker_synced_at, Some(44));
        assert!(refreshed.worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().rank,
            row.snapshot.items[0].rank
        );
        // Replaying the older list cannot roll content back or duplicate the Task.
        store.seed_inventory(&row).unwrap();
        assert_eq!(store.task(&task.id).unwrap().unwrap().title, detail.name);
        assert_eq!(
            store.task_tracker_state(&task.id).unwrap().synced_at,
            Some(44)
        );
        store
            .record_tracker_event(&detail.id, Some("2099-01-03T00:00:00Z"), false)
            .unwrap();
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_tracker_state(&task.id).unwrap(),
            crate::store::TrackerState {
                state: PlanningState::Invalid,
                synced_at: Some(44),
            }
        );
        assert!(store.validate_task_planning(&refreshed).is_err());
        assert_eq!(store.task(&task.id).unwrap().unwrap(), refreshed);
        store
            .record_tracker_event(&row.snapshot.items[0].id, None, true)
            .unwrap();
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_by_issue("MOVED-42").unwrap().unwrap().id,
            task.id
        );
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_ownership", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn team_change_at_the_stored_revision_follows_linear() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        crate::work::wave::bootstrap::upsert_wave(&store, &wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let mut project = snapshot.projects[0].clone();
        let issue = snapshot.items[0].clone();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                initiative: project.initiative_ids[0].clone(),
                synced_at: 42,
                snapshot,
            })
            .unwrap();
        let task = store.task_by_issue(&issue.id).unwrap().unwrap();
        let state = || store.task_tracker_state(&task.id).unwrap().state;
        let teams = || -> String {
            store
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT team_ids FROM tracker_projects", [], |row| {
                    row.get(0)
                })
                .unwrap()
        };

        // Linear adds a Team without moving the Project's `updatedAt`.
        project.team_ids.push("another-team".into());
        loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
            &store,
            "/repo",
            &issue,
            Some(&project),
            43,
            None,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
        assert_eq!(teams(), serde_json::to_string(&project.team_ids).unwrap());
        assert_eq!(state(), PlanningState::Available);

        // An Initiative move is held, and released once Linear moves it back.
        let initiative = project.initiative_ids[0].clone();
        project.initiative_ids = vec!["another-initiative".into()];
        let error = loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
            &store,
            "/repo",
            &issue,
            Some(&project),
            44,
            None,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains(
                "moved to another Initiative in Linear; move it back, then run `lf wave sync`"
            ),
            "{error}"
        );
        assert_eq!(state(), PlanningState::Invalid);
        project.initiative_ids = vec![initiative];
        loopflow_storage::sqlite::linear_events::accept_linear_issue_in(
            &store,
            "/repo",
            &issue,
            Some(&project),
            45,
            None,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
        .unwrap();
        assert_eq!(state(), PlanningState::Available);
    }
}
