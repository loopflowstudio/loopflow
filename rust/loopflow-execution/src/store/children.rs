//! Durable Project and Task rows and their events.

use std::sync::Arc;

use loopflow_model::id::WaveId;
use loopflow_storage::work::project::{Project, ProjectEvent, ProjectEventKind, ProjectId};
use loopflow_storage::work::task::{
    Task, TaskEvent, TaskEventKind, TaskId, TaskPr, TaskPrId, TaskTrackerCursor,
    TrackerEventOutcome,
};
use time::OffsetDateTime;

use super::{run_planning_write, run_sqlite, Store, StoreResult};

impl Store {
    #[allow(clippy::too_many_arguments)] // These effects share the promotion transaction.
    pub(crate) async fn promote_task(
        &self,
        task_id: &TaskId,
        creation: Option<loopflow_storage::work::task::NewTask>,
        placement: Option<(std::path::PathBuf, TaskPr)>,
        session: Option<crate::session::LfSession>,
        destination: Option<std::path::PathBuf>,
        acquisition: Option<Arc<super::PlanningLocks>>,
        receipt: Option<super::sqlite::creation::TaskCreationRecord>,
    ) -> StoreResult<Task> {
        let task_id = task_id.clone();
        run_planning_write(&self.sqlite, acquisition, move |store| {
            crate::store::ownership::promote_task(
                &store,
                &task_id,
                creation.as_ref(),
                placement.as_ref(),
                session.as_ref(),
                destination.as_deref(),
                receipt.as_ref(),
            )
        })
        .await
    }

    pub async fn edit_task(
        &self,
        id: &TaskId,
        expected_revision: u64,
        patch: &loopflow_storage::work::task::TaskUpdate,
    ) -> StoreResult<Task> {
        let id = id.clone();
        let patch = patch.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.edit_task(&id, expected_revision, &patch)
        })
        .await
    }

    pub async fn task_checkout(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<super::sqlite::TaskCheckout>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_checkout(&task)).await
    }

    pub async fn task_checkouts(&self) -> StoreResult<Vec<super::sqlite::TaskCheckout>> {
        run_sqlite(&self.sqlite, |store| store.task_checkouts()).await
    }
    pub(crate) async fn lock_checkout_roots(
        &self,
        roots: Vec<std::path::PathBuf>,
    ) -> StoreResult<Vec<Arc<std::fs::File>>> {
        run_sqlite(&self.sqlite, move |store| {
            super::admission::lock_checkout_roots(&store, &roots)
                .map(|locks| locks.into_iter().map(Arc::new).collect())
        })
        .await
    }

    pub async fn place_task(
        &self,
        task_id: &TaskId,
        worktree: &std::path::Path,
        workspace_slug: &str,
        pr: &TaskPr,
        acquisition: Option<Arc<super::PlanningLocks>>,
    ) -> StoreResult<Task> {
        let task_id = task_id.clone();
        let worktree = worktree.to_path_buf();
        let workspace_slug = workspace_slug.to_string();
        let pr = pr.clone();
        run_planning_write(&self.sqlite, acquisition, move |store| {
            crate::store::ownership::place_task(&store, &task_id, &worktree, &workspace_slug, &pr)
        })
        .await
    }

    pub(crate) async fn complete_task(&self, task: &Task, request: i64) -> StoreResult<bool> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_task(&task, request)
        })
        .await
    }

    pub async fn get_task(&self, task_id: &TaskId) -> StoreResult<Option<Task>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task(&task_id)).await
    }

    pub async fn get_task_by_issue(&self, issue: &str) -> StoreResult<Option<Task>> {
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| store.task_by_issue(&issue)).await
    }

    pub async fn get_task_by_branch(&self, branch: &str) -> StoreResult<Option<Task>> {
        let branch = branch.to_string();
        run_sqlite(&self.sqlite, move |store| store.task_by_branch(&branch)).await
    }

    pub async fn list_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_tasks(wave_id.as_ref())
        })
        .await
    }

    pub async fn list_open_tasks(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Task>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_open_tasks(wave_id.as_ref())
        })
        .await
    }

    pub async fn stack_task_placement(
        &self,
        task: &Task,
        parent: &TaskPrId,
        base_commit: &str,
    ) -> StoreResult<()> {
        let task = task.clone();
        let parent = parent.clone();
        let base_commit = base_commit.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.stack_task_placement(&task, &parent, &base_commit)
        })
        .await
    }

    pub async fn sync_task_placement(
        &self,
        task_id: &TaskId,
        new_base: &str,
        parent_pr_id: Option<&TaskPrId>,
        updated_at: OffsetDateTime,
    ) -> StoreResult<()> {
        let task_id = task_id.clone();
        let new_base = new_base.to_string();
        let parent_pr_id = parent_pr_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.sync_task_placement(&task_id, &new_base, parent_pr_id.as_ref(), updated_at)
        })
        .await
    }

    pub async fn insert_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.insert_task_pr(&pr)).await
    }

    pub async fn update_task_pr(&self, pr: &TaskPr) -> StoreResult<()> {
        let pr = pr.clone();
        run_sqlite(&self.sqlite, move |store| store.update_task_pr(&pr)).await
    }

    pub async fn task_prs(&self, task_id: &TaskId) -> StoreResult<Vec<TaskPr>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_prs(&task_id)).await
    }

    pub async fn latest_task_event_at(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_task_event_at(&task_id)
        })
        .await
    }

    pub async fn recent_task_events(
        &self,
        task_id: &TaskId,
        limit: u32,
    ) -> StoreResult<Vec<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.recent_task_events(&task_id, limit)
        })
        .await
    }

    pub async fn latest_task_event(&self, task_id: &TaskId) -> StoreResult<Option<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.latest_task_event(&task_id)).await
    }

    pub async fn latest_project_event_at(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<OffsetDateTime>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_event_at(&project_id)
        })
        .await
    }

    pub async fn latest_project_event(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<ProjectEvent>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_event(&project_id)
        })
        .await
    }

    pub async fn latest_project_failure(
        &self,
        project_id: &ProjectId,
    ) -> StoreResult<Option<loopflow_storage::work::project::HistoricalFailure>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.latest_project_failure(&project_id)
        })
        .await
    }

    pub async fn get_task_pr(&self, pr_id: &TaskPrId) -> StoreResult<Option<TaskPr>> {
        let pr_id = pr_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_pr(&pr_id)).await
    }

    pub async fn active_task_pr(&self, task_id: &TaskId) -> StoreResult<Option<TaskPr>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| store.active_task_pr(&task_id)).await
    }

    pub async fn settle_task_pr(&self, settled: &TaskPr) -> StoreResult<()> {
        let settled = settled.clone();
        run_sqlite(&self.sqlite, move |store| store.settle_task_pr(&settled)).await
    }

    pub async fn settle_task_pr_merged(
        &self,
        settled: &TaskPr,
        merged_at: Option<OffsetDateTime>,
    ) -> StoreResult<crate::store::TaskPrMergeEvidenceOutcome> {
        let settled = settled.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.settle_task_pr_merged(&settled, merged_at)
        })
        .await
    }

    pub async fn task_tracker_cursor(
        &self,
        task_id: &TaskId,
    ) -> StoreResult<Option<TaskTrackerCursor>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_tracker_cursor(&task_id)
        })
        .await
    }

    pub(crate) async fn apply_tracker_event(
        &self,
        task_id: &TaskId,
        event: loopflow_storage::planning::TrackerEvent,
        observed_at: OffsetDateTime,
        planning_connected: bool,
    ) -> StoreResult<TrackerEventOutcome> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            crate::store::ownership::apply_tracker_event(
                &store,
                &task_id,
                &event,
                observed_at,
                planning_connected,
            )
        })
        .await
    }

    pub async fn mark_task_linear_degraded(
        &self,
        task_id: &TaskId,
        reason: String,
    ) -> StoreResult<()> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_task_linear_degraded(&task_id, &reason)
        })
        .await
    }

    pub async fn append_task_event(
        &self,
        task_id: &TaskId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let task_id = task_id.clone();
        let kind = kind.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.append_task_event(&task_id, &kind)
        })
        .await
    }

    pub async fn task_events_after(
        &self,
        task_id: &TaskId,
        cursor: i64,
    ) -> StoreResult<Vec<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_events_after(&task_id, cursor)
        })
        .await
    }

    pub async fn get_task_event(
        &self,
        task_id: &TaskId,
        event_id: i64,
    ) -> StoreResult<Option<TaskEvent>> {
        let task_id = task_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_event(&task_id, event_id)
        })
        .await
    }

    pub async fn create_project(&self, project: &Project) -> StoreResult<()> {
        let project = project.clone();
        run_sqlite(&self.sqlite, move |store| store.insert_project(&project)).await
    }

    pub async fn get_project(&self, project_id: &ProjectId) -> StoreResult<Option<Project>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| store.project(&project_id)).await
    }

    pub async fn get_project_by_project(&self, project: &str) -> StoreResult<Option<Project>> {
        let project = project.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.project_by_project(&project)
        })
        .await
    }

    pub async fn list_projects(&self, wave_id: Option<&WaveId>) -> StoreResult<Vec<Project>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_projects(wave_id.as_ref())
        })
        .await
    }

    pub async fn append_project_event(
        &self,
        project_id: &ProjectId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let project_id = project_id.clone();
        let kind = kind.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.append_project_event(&project_id, &kind)
        })
        .await
    }

    pub async fn project_events_after(
        &self,
        project_id: &ProjectId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let project_id = project_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_events_after(&project_id, cursor)
        })
        .await
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Store {
    pub async fn seed_task(&self, task: &Task, pr: &TaskPr) -> StoreResult<Task> {
        let me = task
            .worktree
            .is_some()
            .then(|| {
                crate::store::ownership::acquire(
                    &self.sqlite,
                    self.sqlite
                        .new_task_scope(&task.repository_id, task.project_id.as_ref())?,
                )
            })
            .transpose()?;
        self.sqlite.seed_task(task, pr, me.as_ref())
    }
}

/// Observe repository aliases outside SQL, then resolve against the same saved addresses.
pub fn resolve_task_id(
    store: &super::sqlite::SqliteStore,
    issue: &str,
    repo: Option<&str>,
) -> StoreResult<Option<TaskId>> {
    let Some(repo) = repo else {
        return store.resolve_task_id(issue, None);
    };
    loop {
        let saved = store.task_identity_candidates(issue)?;
        let canonical = crate::repository::discover(std::path::Path::new(repo)).ok();
        let matching = saved
            .iter()
            .filter_map(|(_, _, owner)| owner.as_ref())
            .filter(|owner| {
                owner.as_str() == repo
                    || canonical.as_ref().is_some_and(|canonical| {
                        crate::repository::discover(std::path::Path::new(owner))
                            .is_ok_and(|other| &other == canonical)
                    })
            })
            .cloned()
            .collect();
        let scope = super::sqlite::children::TaskRepositoryScope { saved, matching };
        match store.resolve_task_id(issue, Some(&scope)) {
            Err(super::StoreError::Stale(_)) => continue,
            result => return result,
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use super::resolve_task_id;
    use loopflow_test_support::TestRepo;

    #[test]
    fn a_task_prefix_resolves_in_its_repository_from_a_linked_checkout() {
        let repo = TestRepo::new();
        let checkout = repo.create_named_worktree("scope-child");
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let first = "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
        let second = "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2";
        // Historical addresses can name a linked checkout rather than its main root.
        for (repo_id, path, task) in [
            (
                "repo_11111111111111111111111111111111",
                checkout.display().to_string(),
                first,
            ),
            (
                "repo_22222222222222222222222222222222",
                "/other-repository".into(),
                second,
            ),
        ] {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,?2,1)",
                    rusqlite::params![repo_id, path],
                )
                .unwrap();
            store
                .create_task(&loopflow_storage::work::task::NewTask {
                    id: loopflow_model::durable::TaskId::parse(task).unwrap(),
                    repository_id: loopflow_model::durable::RepositoryId::parse(repo_id).unwrap(),
                    project_id: None,
                    title: task.into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
        }
        assert!(resolve_task_id(&store, "lf-aaaaaa", None).is_err());
        for requested in [repo.path(), checkout.as_path()] {
            let selected = resolve_task_id(&store, "lf-aaaaaa", Some(requested.to_str().unwrap()))
                .unwrap()
                .unwrap();
            assert_eq!(selected.as_str(), first);
            assert_eq!(store.task(&selected).unwrap().unwrap().id, selected);
        }
        assert!(
            resolve_task_id(&store, "lf-aaaaaa", Some("/missing-repository"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            resolve_task_id(&store, "lf-aaaaaa", Some("/other-repository"))
                .unwrap()
                .unwrap()
                .as_str(),
            second
        );
    }

    #[test]
    fn changed_task_identity_candidates_reject_a_stale_lookup() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let scope = loopflow_storage::sqlite::children::TaskRepositoryScope {
            saved: store.task_identity_candidates("lf-aaaaaa").unwrap(),
            matching: vec!["/repo".into()],
        };
        store.conn.lock().unwrap().execute("INSERT INTO repository_plans(id,repo,selected) VALUES('repo_11111111111111111111111111111111','/repo',1)", []).unwrap();
        store.conn.lock().unwrap().execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at) VALUES('task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','repo_11111111111111111111111111111111','LOOKUP-1',1)", []).unwrap();
        assert!(matches!(
            store.resolve_task_id("lf-aaaaaa", Some(&scope)),
            Err(crate::store::StoreError::Stale(_))
        ));
    }
}

#[cfg(test)]
#[path = "children_tests.rs"]
mod tests;
