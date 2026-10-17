//! Saved selection and layout intent share one transaction and one Work snapshot.
use super::SqliteStore;
use crate::durable::RepositoryId;
use crate::store::{StoreError, StoreResult};
use crate::work::selection::{
    Arrangement, QueryId, SavedSelection, Selection, SelectionReading, SelectionResolution,
    SelectionWork, WorkWorkspaceKey, WorkWorkspaceKind,
};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::BTreeMap;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn catalog_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<Vec<SavedSelection>> {
    let mut statement = conn.prepare("SELECT id,revision,selection_json,arrangement_json FROM saved_selections WHERE repository_id=?1 ORDER BY id")?;
    let rows = statement.query_map([repository.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (id, revision, selection, arrangement) = row?;
        Ok(SavedSelection {
            repository: repository.to_string(),
            id: QueryId(id),
            revision,
            selection: serde_json::from_str(&selection)?,
            arrangement: serde_json::from_str(&arrangement)?,
        })
    })
    .collect()
}

fn work_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<Vec<SelectionWork>> {
    let repo: String = conn.query_row(
        "SELECT repo FROM repository_plans WHERE id=?1",
        [repository.as_str()],
        |row| row.get(0),
    )?;
    let mut waves = super::waves_in(conn, Some(&repo))?;
    waves.sort_by(|a, b| (a.name(), a.id().as_str()).cmp(&(b.name(), b.id().as_str())));
    let key = |kind, id| WorkWorkspaceKey {
        repository: repository.to_string(),
        kind,
        id,
    };
    let mut work = vec![SelectionWork {
        key: key(WorkWorkspaceKind::Repository, repository.to_string()),
        parent_wave: None,
        open: None,
    }];
    for wave in &waves {
        work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
            parent_wave: wave.parent_wave_id().cloned(),
            open: None,
        });
    }
    let mut tasks = Vec::new();
    for wave in &waves {
        tasks.extend(super::children::tasks_in(conn, Some(wave.id()))?);
    }
    tasks.sort_by(|a, b| (a.rank, a.id.as_str()).cmp(&(b.rank, b.id.as_str())));
    for task in tasks {
        let open = super::chapters::task_started_in(conn, &task.id)?
            && !super::durable::task_state_in(conn, &task.id)?.is_terminal();
        work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Task, task.id.to_string()),
            parent_wave: Some(task.wave_id),
            open: Some(open),
        });
    }
    Ok(work)
}

fn resolve_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: &Selection,
    catalog: &[SavedSelection],
) -> StoreResult<SelectionResolution> {
    selection
        .resolve(
            repository.as_str(),
            &work_in(conn, repository)?,
            &definitions(catalog),
        )
        .map_err(invalid)
}

fn definitions(catalog: &[SavedSelection]) -> BTreeMap<QueryId, Selection> {
    catalog
        .iter()
        .map(|saved| (saved.id.clone(), saved.selection.clone()))
        .collect()
}

impl SqliteStore {
    /// A read composes with the Work observer's pinned snapshot. No acquisition.
    pub fn resolve_selection(
        &self,
        repository: &RepositoryId,
        selection: &Selection,
    ) -> StoreResult<SelectionResolution> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        resolve_in(
            &snapshot,
            repository,
            selection,
            &catalog_in(&snapshot, repository)?,
        )
    }

    pub fn saved_selections(&self, repository: &RepositoryId) -> StoreResult<Vec<SavedSelection>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        catalog_in(&conn, repository)
    }

    pub fn read_selection(
        &self,
        repository: &RepositoryId,
        id: &QueryId,
    ) -> StoreResult<Option<SelectionReading>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let catalog = catalog_in(&snapshot, repository)?;
        let Some(saved) = catalog.iter().find(|saved| &saved.id == id) else {
            return Ok(None);
        };
        let resolution = resolve_in(&snapshot, repository, &saved.selection, &catalog)?;
        let mut arrangement = saved.arrangement.clone();
        arrangement.reconcile(&resolution);
        Ok(Some(SelectionReading {
            saved: saved.clone(),
            resolution,
            arrangement,
        }))
    }

    /// Compare-and-swap both authored values. A stale or invalid edit writes nothing.
    pub fn save_selection(
        &self,
        repository: &RepositoryId,
        id: &QueryId,
        expected_revision: Option<i64>,
        selection: &Selection,
        arrangement: &Arrangement,
    ) -> StoreResult<SavedSelection> {
        if id.0.is_empty() {
            return Err(invalid("query ID must be nonempty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut catalog = catalog_in(&tx, repository)?;
        let prior = catalog.iter().find(|saved| &saved.id == id).cloned();
        if prior.as_ref().map(|saved| saved.revision) != expected_revision {
            return Err(invalid(
                "saved selection changed; read it again before editing",
            ));
        }
        let saved = SavedSelection {
            repository: repository.to_string(),
            id: id.clone(),
            revision: expected_revision.unwrap_or(0) + 1,
            selection: selection.clone(),
            arrangement: arrangement.clone(),
        };
        catalog.retain(|saved| &saved.id != id);
        catalog.push(saved.clone());
        let work = work_in(&tx, repository)?;
        let definitions = definitions(&catalog);
        let resolution = selection
            .resolve(repository.as_str(), &work, &definitions)
            .map_err(invalid)?;
        // Validate dependents against the same facts; an edit must not introduce a cycle.
        for entry in catalog.iter().filter(|entry| &entry.id != id) {
            entry
                .selection
                .resolve(repository.as_str(), &work, &definitions)
                .map_err(invalid)?;
        }
        let mut expected =
            prior.map_or_else(|| Arrangement::automatic(&[]), |saved| saved.arrangement);
        expected.reconcile(&resolution);
        arrangement.validate(&expected.members()).map_err(invalid)?;
        tx.execute("INSERT INTO saved_selections(repository_id,id,revision,selection_json,arrangement_json) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(repository_id,id) DO UPDATE SET revision=excluded.revision,selection_json=excluded.selection_json,arrangement_json=excluded.arrangement_json",
            params![repository.as_str(),id.0,saved.revision,serde_json::to_string(selection)?,serde_json::to_string(arrangement)?])?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn delete_selection(
        &self,
        repository: &RepositoryId,
        id: &QueryId,
        expected_revision: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM saved_selections WHERE repository_id=?1 AND id=?2 AND revision=?3",
            params![repository.as_str(), id.0, expected_revision],
        )?;
        if deleted == 0 {
            return Err(invalid(
                "saved selection changed; read it again before deleting",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::durable::RepositoryId;
    use crate::work::selection::{
        Arrangement, ArrangementTree, QueryId, Selection, SelectionExpression, SelectionId,
        WorkWorkspaceKind,
    };

    fn fixture() -> (tempfile::TempDir, SqliteStore, RepositoryId) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!(
                "../../../tests/fixtures/migrations/saved_selection_work.sql"
            ))
            .unwrap();
        let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
        (dir, store, repo)
    }

    fn selection() -> Selection {
        Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Repository,
        }
    }

    #[test]
    fn released_work_upgrades_to_saved_intent_without_rewriting_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "saved_selections");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/saved_selection_work.sql"
        ))
        .unwrap();
        conn.execute_batch(&crate::store::migrations::current_draft_sql(
            "saved_selections",
        ))
        .unwrap();
        drop(conn);
        let store = SqliteStore {
            conn: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open(&path).unwrap(),
            )),
        };
        let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
        let selection = selection();
        let result = store.resolve_selection(&repo, &selection).unwrap();
        assert_eq!(result.members.len(), 4);
        let id = QueryId::new();
        let saved = store
            .save_selection(
                &repo,
                &id,
                None,
                &selection,
                &Arrangement::automatic(&result.members),
            )
            .unwrap();
        let reader = SqliteStore::open_read_only(&path).unwrap();
        assert_eq!(
            reader.read_selection(&repo, &id).unwrap().unwrap().saved,
            saved
        );
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            4
        );
        crate::store::migrations::validate_persisted_json(&conn).unwrap();
    }

    #[test]
    fn saved_selections_upgrade_reload_and_edit_atomically_without_execution() {
        let (_dir, store, repo) = fixture();
        let selection = selection();
        let id = QueryId::new();
        let result = store.resolve_selection(&repo, &selection).unwrap();
        assert!(result.unavailable.is_empty());
        assert_eq!(result.members.len(), 4); // Two Waves, paused and idle Tasks.
        assert_eq!(
            result
                .members
                .iter()
                .filter(|member| member.work.kind == WorkWorkspaceKind::Task)
                .map(|member| member.work.id.as_str())
                .collect::<Vec<_>>(),
            [
                "task_11111111111111111111111111111111",
                "task_22222222222222222222222222222222"
            ]
        );
        let hidden = result.members[0].clone();
        let mut arrangement = Arrangement::automatic(&result.members);
        arrangement.hide(&hidden, true).unwrap();
        let first = store
            .save_selection(&repo, &id, None, &selection, &arrangement)
            .unwrap();
        // A second connection reads real lfdb bytes, without opening a native surface.
        let reader = SqliteStore::open_read_only(&_dir.path().join("db")).unwrap();
        let loaded = reader.read_selection(&repo, &id).unwrap().unwrap();
        assert_eq!(loaded.saved, first);
        assert_eq!(loaded.arrangement, arrangement);
        let mut edited = arrangement.tree.clone().unwrap();
        if let ArrangementTree::Split { ratio, .. } = &mut edited {
            *ratio = 0.7;
        }
        arrangement.replace(Some(edited)).unwrap();
        let saved = store
            .save_selection(&repo, &id, Some(first.revision), &selection, &arrangement)
            .unwrap();
        assert!(store
            .save_selection(&repo, &id, Some(first.revision), &selection, &arrangement)
            .is_err());
        let incomplete = Arrangement::automatic(&result.members[1..]);
        assert!(store
            .save_selection(&repo, &id, Some(saved.revision), &selection, &incomplete)
            .is_err());
        assert_eq!(reader.saved_selections(&repo).unwrap(), [saved]);
        let mut restored = reader
            .read_selection(&repo, &id)
            .unwrap()
            .unwrap()
            .arrangement;
        restored.hide(&hidden, false).unwrap();
        assert_eq!(restored.tree, arrangement.tree);
        let conn = store.conn.lock().unwrap();
        for table in ["agent_sessions", "processes", "flow_processes"] {
            assert_eq!(
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        crate::store::migrations::validate_persisted_json(&conn).unwrap();
    }

    #[test]
    fn nested_query_failure_retains_prior_members_but_confirmed_departure_prunes() {
        let (_dir, store, repo) = fixture();
        let base = selection();
        let base_id = QueryId::new();
        let resolution = store.resolve_selection(&repo, &base).unwrap();
        store
            .save_selection(
                &repo,
                &base_id,
                None,
                &base,
                &Arrangement::automatic(&resolution.members),
            )
            .unwrap();
        let nested = Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Query {
                query: base_id.clone(),
            },
        };
        let nested_id = QueryId::new();
        let result = store.resolve_selection(&repo, &nested).unwrap();
        let mut arrangement = Arrangement::automatic(&result.members);
        arrangement.replace(arrangement.tree.clone()).unwrap();
        store
            .save_selection(&repo, &nested_id, None, &nested, &arrangement)
            .unwrap();
        // The named query edit would introduce a cycle through the dependent.
        let cycle = Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Query {
                query: nested_id.clone(),
            },
        };
        assert!(store
            .save_selection(&repo, &base_id, Some(1), &cycle, &arrangement)
            .is_err());
        assert!(store.delete_selection(&repo, &base_id, 2).is_err());
        assert_eq!(
            store
                .read_selection(&repo, &nested_id)
                .unwrap()
                .unwrap()
                .arrangement,
            arrangement
        );
        assert!(store.read_selection(&repo, &base_id).unwrap().is_some());
        store.delete_selection(&repo, &base_id, 1).unwrap();
        assert!(store.read_selection(&repo, &base_id).unwrap().is_none());
        assert!(store.delete_selection(&repo, &base_id, 1).is_err());
        let unavailable = store.read_selection(&repo, &nested_id).unwrap().unwrap();
        assert!(!unavailable.resolution.unavailable.is_empty());
        assert_eq!(unavailable.arrangement, arrangement);
        store
            .save_selection(
                &repo,
                &base_id,
                None,
                &base,
                &Arrangement::automatic(&resolution.members),
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=1 WHERE issue_identifier='FIX-2'",
                [],
            )
            .unwrap();
        let current = store.read_selection(&repo, &nested_id).unwrap().unwrap();
        assert!(current.resolution.unavailable.is_empty());
        assert_eq!(current.arrangement.members().len(), 3);
        assert_eq!(current.saved.arrangement.members().len(), 4); // Reading does not write.
    }

    #[test]
    fn selection_reads_compose_with_the_work_snapshot_and_report_failed_reads() {
        let (_dir, store, repo) = fixture();
        let _snapshot = store.read_snapshot().unwrap();
        let result = store.resolve_selection(&repo, &selection()).unwrap();
        assert_eq!(result.members.len(), 4);
        let missing = RepositoryId::new();
        assert!(store.resolve_selection(&missing, &selection()).is_err());
    }
}
