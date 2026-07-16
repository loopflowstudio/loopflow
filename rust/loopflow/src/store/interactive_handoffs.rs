use crate::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffId, InteractiveHandoffOutcome, InteractiveHandoffParent,
    OpenInteractiveHandoff,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    /// Open the parent's one unresolved interactive handoff. Replays return the
    /// existing row and `false`; a new row returns `true`.
    pub async fn open_interactive_handoff(
        &self,
        request: OpenInteractiveHandoff,
    ) -> StoreResult<(InteractiveHandoff, bool)> {
        run_sqlite(&self.sqlite, move |store| {
            store.open_interactive_handoff(&request)
        })
        .await
    }

    pub async fn get_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
    ) -> StoreResult<Option<InteractiveHandoff>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.get_interactive_handoff(&session_id)
        })
        .await
    }

    pub async fn list_interactive_handoffs(
        &self,
        parent: Option<&InteractiveHandoffParent>,
    ) -> StoreResult<Vec<InteractiveHandoff>> {
        let parent = parent.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_interactive_handoffs(parent.as_ref())
        })
        .await
    }

    /// Record first attach evidence and return the same Session on every read.
    pub async fn attach_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
    ) -> StoreResult<InteractiveHandoff> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.attach_interactive_handoff(&session_id)
        })
        .await
    }

    /// Record the first terminal outcome. Repeating that exact outcome is
    /// idempotent; a conflicting stale outcome is an error.
    pub async fn finish_interactive_handoff(
        &self,
        session_id: &InteractiveHandoffId,
        outcome: &InteractiveHandoffOutcome,
    ) -> StoreResult<InteractiveHandoff> {
        let session_id = session_id.clone();
        let outcome = outcome.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.finish_interactive_handoff(&session_id, &outcome)
        })
        .await
    }

    /// Claim the terminal handoff's one parent wake. The generation is the
    /// current parent body, which may differ from the body that opened it.
    pub async fn claim_interactive_handoff_wake(
        &self,
        session_id: &InteractiveHandoffId,
        parent_generation: u32,
    ) -> StoreResult<bool> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_interactive_handoff_wake(&session_id, parent_generation)
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use time::OffsetDateTime;

    use crate::child_session::{ChildLeaseState, ChildProcessGeneration};
    use crate::engine::wave_home::WaveHome;
    use crate::id::WaveId;
    use crate::interactive_handoff::{
        InteractiveHandoff, InteractiveHandoffOutcome, InteractiveHandoffParent,
        InteractiveHandoffStatus, OpenInteractiveHandoff,
    };
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, StorageConfig, Store};
    use crate::task::{
        PmWritebackState, TaskPr, TaskPrId, TaskSession, TaskSessionId, TaskSessionStatus,
    };
    use crate::wave::Wave;

    fn process(generation: u32, provider_session_id: &str) -> ChildProcessGeneration {
        ChildProcessGeneration {
            generation,
            pid: Some(42),
            process_group_id: Some(42),
            tmux_name: "lf-task-body".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some(provider_session_id.to_string()),
            started_at: OffsetDateTime::now_utc(),
            state: ChildLeaseState::Active,
            outcome: None,
            provenance: None,
        }
    }

    async fn task_parent(store: &Store, repo: &Path) -> TaskSession {
        let now = OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project_snapshot = LinearProjectSnapshot {
            id: LinearProjectId::new("project-1").unwrap(),
            slug: "loopflow-api".to_string(),
            name: "Loopflow API".to_string(),
            prompt_context: "Keep one model everywhere.".to_string(),
        };
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: project_snapshot.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 1,
            incorporated_directive_version: 1,
            status: ProjectSessionStatus::Running,
            status_reason: "project body active".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("project-thread".to_string()),
            latest_process: Some(process(1, "project-thread")),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&project).await.unwrap();

        let task_id = TaskSessionId::new();
        let task = TaskSession {
            id: task_id.clone(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-1").unwrap(),
                    identifier: "W2-175".to_string(),
                    title: "Hand interactive work to a human".to_string(),
                    description: "Preserve the same provider history.".to_string(),
                },
                project: project_snapshot,
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id,
            current_directive_version: 1,
            incorporated_directive_version: 1,
            status: TaskSessionStatus::Running,
            status_reason: "task body active".to_string(),
            status_at: now,
            worktree: repo.to_path_buf(),
            workspace_slug: "interactive-handoff".to_string(),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("task-thread".to_string()),
            latest_process: Some(process(4, "task-thread")),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: task_id,
            sequence: 1,
            slug: "interactive-handoff".to_string(),
            branch: "jack/interactive-handoff".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task_session(&task, &pr).await.unwrap();
        task
    }

    fn open_for(parent: InteractiveHandoffParent, cwd: PathBuf) -> OpenInteractiveHandoff {
        OpenInteractiveHandoff {
            parent,
            home: WaveHome::parse("jack@local").unwrap(),
            cwd,
            provider: "codex".to_string(),
            provider_session_id: Some("task-thread".to_string()),
            body_generation: 4,
            reason: "OAuth login requires a human".to_string(),
            environment: BTreeMap::from([
                ("LF_HOME".to_string(), "/tmp/lf".to_string()),
                ("LF_TASK_SESSION_ID".to_string(), "ts-parent".to_string()),
            ]),
            attach_argv: vec![
                "tmux".to_string(),
                "attach-session".to_string(),
                "-t".to_string(),
                "lf-task-interactive".to_string(),
            ],
        }
    }

    #[tokio::test]
    async fn interactive_handoff_survives_repeated_attach_restart_and_parent_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("registry.db");
        let store = open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .unwrap();
        let task = task_parent(&store, dir.path()).await;
        let request = open_for(
            InteractiveHandoffParent::Task(task.id.clone()),
            task.worktree.clone(),
        );
        let mut wrong_worktree = request.clone();
        wrong_worktree.cwd = PathBuf::from("/src/some-other-task");
        assert!(store
            .open_interactive_handoff(wrong_worktree)
            .await
            .is_err());

        let (opened, created) = store
            .open_interactive_handoff(request.clone())
            .await
            .unwrap();
        assert!(created);
        assert_eq!(opened.status, InteractiveHandoffStatus::Waiting);
        let (replayed, created) = store.open_interactive_handoff(request).await.unwrap();
        assert!(!created);
        assert_eq!(replayed.id, opened.id);

        let attached = store.attach_interactive_handoff(&opened.id).await.unwrap();
        let reattached = store.attach_interactive_handoff(&opened.id).await.unwrap();
        assert_eq!(attached.id, reattached.id);
        assert_eq!(attached.attached_at, reattached.attached_at);
        assert_eq!(
            attached.attach_descriptor().argv,
            reattached.attach_descriptor().argv
        );

        drop(store);
        let reopened = open_store(&StorageConfig::sqlite(db_path)).await.unwrap();
        let handed_back = reopened
            .finish_interactive_handoff(
                &opened.id,
                &InteractiveHandoffOutcome::HandedBack {
                    summary: "Finish the review fixes headlessly".to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(handed_back.status, InteractiveHandoffStatus::HandedBack);
        assert!(reopened
            .claim_interactive_handoff_wake(&opened.id, 5)
            .await
            .unwrap());
        assert!(!reopened
            .claim_interactive_handoff_wake(&opened.id, 6)
            .await
            .unwrap());
        let claimed = reopened
            .get_interactive_handoff(&opened.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.wake_claimed_by_generation, Some(5));
    }

    #[tokio::test]
    async fn concurrent_terminal_outcomes_preserve_the_first_result() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("registry.db");
        let first = open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            dir.path().display().to_string(),
        );
        first.create_wave(&wave).await.unwrap();
        let mut request = open_for(
            InteractiveHandoffParent::Wave(wave.id().clone()),
            dir.path().to_path_buf(),
        );
        request.provider_session_id = None;
        request.body_generation = 1;
        let (handoff, _) = first.open_interactive_handoff(request).await.unwrap();
        let second = open_store(&StorageConfig::sqlite(db_path)).await.unwrap();

        let completed = InteractiveHandoffOutcome::Completed {
            summary: "Human finished the work".to_string(),
        };
        let failed = InteractiveHandoffOutcome::Failed {
            reason: "interactive body disappeared".to_string(),
        };
        let (left, right) = tokio::join!(
            first.finish_interactive_handoff(&handoff.id, &completed),
            second.finish_interactive_handoff(&handoff.id, &failed),
        );
        assert_ne!(left.is_ok(), right.is_ok());
        let persisted = first
            .get_interactive_handoff(&handoff.id)
            .await
            .unwrap()
            .unwrap();
        let winning = left.ok().or_else(|| right.ok()).unwrap();
        assert_eq!(persisted.outcome, winning.outcome);
    }

    #[tokio::test]
    async fn failed_interactive_body_is_terminal_and_wakes_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".to_string(),
            dir.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let mut request = open_for(
            InteractiveHandoffParent::Wave(wave.id().clone()),
            dir.path().to_path_buf(),
        );
        request.provider_session_id = None;
        request.body_generation = 1;
        let (handoff, _) = store.open_interactive_handoff(request).await.unwrap();
        assert!(store
            .claim_interactive_handoff_wake(&handoff.id, 1)
            .await
            .is_err());

        let outcome = InteractiveHandoffOutcome::Failed {
            reason: "tmux session exited before completion".to_string(),
        };
        let failed = store
            .finish_interactive_handoff(&handoff.id, &outcome)
            .await
            .unwrap();
        assert_eq!(failed.status, InteractiveHandoffStatus::Failed);
        assert_eq!(
            store
                .finish_interactive_handoff(&handoff.id, &outcome)
                .await
                .unwrap()
                .outcome,
            Some(outcome)
        );
        assert!(store
            .claim_interactive_handoff_wake(&handoff.id, 2)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn list_row_maps_census_fields_and_active_filter_drops_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let task = task_parent(&store, dir.path()).await;
        let (handoff, _) = store
            .open_interactive_handoff(open_for(
                InteractiveHandoffParent::Task(task.id.clone()),
                task.worktree.clone(),
            ))
            .await
            .unwrap();

        let now = OffsetDateTime::now_utc();
        let row = handoff.list_row(now);
        assert_eq!(row.session_id, handoff.id);
        assert_eq!(row.parent_kind, "task");
        assert_eq!(row.parent_id, task.id.as_str());
        assert_eq!(row.wave_id, task.wave_id);
        assert_eq!(row.status, InteractiveHandoffStatus::Waiting);
        assert_eq!(row.home, "jack@local");
        assert_eq!(row.provider, "codex");
        // Age is measured, never fabricated; a just-opened row is a few seconds old.
        assert!(row.age_secs.is_some_and(|age| age >= 0));

        // A waiting handoff is active; a terminal one is filtered by `--active`.
        assert!(handoff.is_active());
        let all = store.list_interactive_handoffs(None).await.unwrap();
        assert_eq!(all.len(), 1);
        let finished = store
            .finish_interactive_handoff(
                &handoff.id,
                &InteractiveHandoffOutcome::Completed {
                    summary: "human logged in".to_string(),
                },
            )
            .await
            .unwrap();
        assert!(!finished.is_active());
        let active: Vec<_> = store
            .list_interactive_handoffs(None)
            .await
            .unwrap()
            .into_iter()
            .filter(InteractiveHandoff::is_active)
            .collect();
        assert!(active.is_empty());
    }
}
