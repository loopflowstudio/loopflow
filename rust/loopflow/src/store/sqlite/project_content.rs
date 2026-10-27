//! Project content saves in one transaction in every repository.

use crate::work::edit::WorkValue as Edit;
use rusqlite::TransactionBehavior;

use crate::durable::ProjectId;
use crate::store::planning_sync::PlanningChange;
use crate::store::{StoreError, StoreResult};
use crate::work::project::ProjectKR;
use crate::work::wave::metrics::MetricTarget;

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

impl SqliteStore {
    pub fn pending_project_changes(&self, project: &ProjectId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        PlanningChanges::Project(project).pending(&conn)
    }

    pub fn edit_project(
        &self,
        project: &ProjectId,
        name: Option<&str>,
        summary: Option<&str>,
        description: Option<&str>,
    ) -> StoreResult<()> {
        if name.is_some_and(|name| name.trim().is_empty()) {
            return Err(StoreError::InvalidData(
                "Project name cannot be empty".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let edits: Vec<_> = name
            .map(|name| Edit::ProjectName(Some(name.into())))
            .into_iter()
            .chain(summary.map(|summary| Edit::ProjectSummary(summary.into())))
            .chain(
                description
                    .map(|description| Edit::ProjectDescription(Some(description.trim().into()))),
            )
            .collect();
        PlanningChanges::Project(project).save(&tx, &edits)?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_project_content(
        &self,
        project: &ProjectId,
        krs: &[ProjectKR],
        metric_targets: &[MetricTarget],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        PlanningChanges::Project(project).save(
            &tx,
            &[
                Edit::Krs(krs.to_vec()),
                Edit::MetricTargets(metric_targets.to_vec()),
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

    #[test]
    fn project_name_receipt_does_not_reconstruct_unrelated_order_evidence() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let observed = serde_json::from_value(serde_json::json!({
            "id":"linear-project", "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"",
            "workflow":"", "krs":[], "metric_targets":[], "status":"started", "initiative_ids":["initiative"], "team_ids":["team"]
        })).unwrap();
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            17,
        )
        .unwrap();
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE pm_projects SET task_order_json='{}' WHERE id='linear-project'",
                [],
            )
            .unwrap();
        store
            .edit_project(&project.id, Some("Renamed"), None, None)
            .unwrap();
        let change = store
            .pending_project_changes(&project.id)
            .unwrap()
            .remove(0);
        assert_eq!(
            serde_json::to_value(&change.base).unwrap(),
            serde_json::json!({"revision":"2026-10-10T12:00:00Z", "value":{"field":"project_name","value":"Product"}})
        );
        assert!(store
            .attempt_planning_field(
                super::super::planning_changes::PlanningChanges::Project(&project.id),
                &change,
                Some("2026-10-10T12:00:00Z"),
                false,
            )
            .unwrap());
        assert_eq!(store.project(&project.id).unwrap().unwrap().name, "Renamed");
        let retained: (String, i64) = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT task_order_json,observed_at FROM pm_projects WHERE id='linear-project'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(retained, ("{}".into(), 17));
    }

    #[test]
    fn project_prose_unobserved_baseline_stays_unknown_at_the_same_revision() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut observed: crate::linear::LinearProject = serde_json::from_value(serde_json::json!({
            "id":"linear-project", "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"",
            "workflow":"", "krs":[], "metric_targets":[], "status":"started", "initiative_ids":["initiative"], "team_ids":["team"]
        })).unwrap();
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            1,
        )
        .unwrap();
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        store
            .edit_project(&project.id, None, None, Some("Saved without a baseline"))
            .unwrap();
        let change = store
            .pending_project_changes(&project.id)
            .unwrap()
            .remove(0);
        assert_eq!(change.base, None);
        observed.description = Some(String::new());
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            2,
        )
        .unwrap();
        // The unsent save is sent after the observed prose, so it wins and
        // rebases onto the value it will replace.
        assert_eq!(
            store.project(&project.id).unwrap().unwrap().description,
            "Saved without a baseline"
        );
        let conn = store.conn.lock().unwrap();
        let (conflict, base): (Option<String>, String) = conn
            .query_row(
                "SELECT conflict_json,base_json FROM project_changes WHERE id=?1",
                [&change.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(conflict.is_none());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&base).unwrap()["value"]["value"],
            ""
        );
    }

    #[test]
    fn project_prose_partial_observation_retains_its_original_baseline_and_age() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut observed: crate::linear::LinearProject = serde_json::from_value(serde_json::json!({
            "id":"linear-project", "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"", "description":"Baseline",
            "workflow":"", "krs":[], "metric_targets":[], "status":"started", "initiative_ids":["initiative"], "team_ids":["team"]
        })).unwrap();
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            17,
        )
        .unwrap();
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        observed.description = None;
        observed.revision = Some("2026-10-10T12:01:00Z".into());
        observed.summary = "New summary".into();
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            29,
        )
        .unwrap();
        let saved = store.project(&project.id).unwrap().unwrap();
        assert_eq!(saved.description, "Baseline");
        assert_eq!(saved.summary, "New summary");
        store
            .edit_project(&project.id, None, None, Some("Next save"))
            .unwrap();
        let change = store
            .pending_project_changes(&project.id)
            .unwrap()
            .remove(0);
        assert_eq!(
            serde_json::to_value(&change.base).unwrap(),
            serde_json::json!({"value":{"field":"project_description","value":"Baseline"}, "revision":"2026-10-10T12:00:00Z"})
        );
        let prose: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT json_object('value',description,'revision',description_revision,'observed_at',description_observed_at) FROM pm_projects WHERE id='linear-project'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&prose).unwrap(),
            serde_json::json!({"value":"Baseline", "revision":"2026-10-10T12:00:00Z", "observed_at":17})
        );
        observed.description = Some("Baseline".into());
        crate::linear::acceptance::accept_linear_project_in(
            &store,
            wave.id(),
            "initiative",
            &observed,
            30,
        )
        .unwrap();
        assert_eq!(
            store.project(&project.id).unwrap().unwrap().description,
            "Next save"
        );
        assert_eq!(
            store.pending_project_changes(&project.id).unwrap(),
            vec![change]
        );
    }

    #[test]
    fn project_edit_failure_rolls_back_fields_definitions_and_receipts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let project = crate::durable::ProjectId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description)
            VALUES(?1,?2,1,1,'product','Original','')",rusqlite::params![project.as_str(),wave.id()]).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_project_save BEFORE UPDATE ON projects
            BEGIN SELECT RAISE(ABORT,'injected write failure'); END;",
        )
        .unwrap();
        drop(conn);
        let before = store.revisions().unwrap();
        assert!(store
            .edit_project(&project, Some("Changed"), Some("New summary"), None)
            .is_err());
        let read = store.project(&project).unwrap().unwrap();
        assert_eq!(read.name, "Original");
        assert_eq!(read.summary, "");
        assert!(store.pending_project_changes(&project).unwrap().is_empty());
        assert_eq!(store.revisions().unwrap(), before);
    }

    #[test]
    fn unchanged_project_content_saves_nothing_with_and_without_linear_mapping() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        for (name, mapping) in [("offline", None), ("connected", Some("linear-project"))] {
            let wave = Wave::new(WaveId::new(), name.into(), "/repo".into());
            store.create_wave(&wave).unwrap();
            let project = crate::durable::ProjectId::new();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at,
                 project_slug,project_name,description) VALUES(?1,?2,?3,1,1,?4,?4,'')",
                    rusqlite::params![project.as_str(), wave.id(), mapping, name],
                )
                .unwrap();
            store.update_project_content(&project, &[], &[]).unwrap();
            let before = store.revisions().unwrap();
            let changes = store.pending_project_changes(&project).unwrap();
            store.update_project_content(&project, &[], &[]).unwrap();
            store
                .edit_project(&project, Some(name), Some(""), None)
                .unwrap();
            assert_eq!(store.revisions().unwrap(), before);
            assert_eq!(store.pending_project_changes(&project).unwrap(), changes);
        }
        assert!(store
            .update_project_content(&crate::durable::ProjectId::new(), &[], &[])
            .is_err());
    }
}
