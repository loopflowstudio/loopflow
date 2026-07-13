//! Persistence as shared infrastructure: the machine's registry, written and
//! read by `lf` directly — `lfd` is one more client, not the owner. This
//! module owns the sqlite registry, schema migrations, and registry API
//! ([`Store`] and its per-domain traits).
//!
//! The persisted domain types still live in `crate::lfd::types` for now; the
//! type split is a later, non-mechanical skill.
//!

use std::path::PathBuf;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, ChatMemoryBlock, ChatMessage,
    LivePullRequestState, Repo, RepoEdge, RepoId, Run, Session, SessionStatus, Summary, Wave,
};
use crate::project_session::{
    ObservationOutboxRow, ProjectCommand, ProjectCommandId, ProjectEvent, ProjectEventKind,
    ProjectSession, ProjectSessionId, ProjectSessionStatus, SessionSupervisor,
};
use crate::task::{
    BoundaryResult, ChildDirective, ChildRef, TaskCommand, TaskCommandId, TaskEvent, TaskEventKind,
    TaskSession, TaskSessionId, TaskSessionStatus,
};

pub mod catalog;
pub mod migrations;
pub mod rows;
pub mod sqlite;
mod token_crypto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ForkRunStatus {
    Pending = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
}

impl ForkRunStatus {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Pending),
            1 => Some(Self::Running),
            2 => Some(Self::Completed),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForkRun {
    pub id: LfdId,
    pub run_id: LfdId,
    pub step_index: u32,
    pub branch_index: u32,
    pub status: ForkRunStatus,
    pub worktree: String,
}

/// One row of the machine-grain run ledger (`run_events`): a lifecycle event
/// for a run, flow, or skill, written directly by `lf` (and by `lfd`) into the
/// local store. Token/cost fields are cumulative snapshots populated on skill
/// boundaries and terminal run events when the stream reported them.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEventRow {
    pub run_id: String,
    pub process_id: String,
    pub parent_process_id: Option<String>,
    pub seq: i64,
    pub ts: i64,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub wave: Option<String>,
    pub node: String,
    pub event: String,
    pub command: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub step_index: Option<i64>,
    pub error: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_secs: Option<f64>,
    /// The harness the tokens were spent through. NULL when the process never
    /// launched an agent.
    pub provider: Option<String>,
    /// The configured model, when the agent launch names one.
    pub model: Option<String>,
}

/// One frame on the agent bus (`bus_messages`). `byline` is testimony — what
/// the publishing client said it was — and `channel` is evidence: where the
/// row actually arrived. Nothing derives identity server-side, so a forged
/// byline shows up as a mismatch between the two rather than being prevented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusMessage {
    /// Monotonic id; a subscriber's cursor is an id it has already read.
    pub id: i64,
    pub channel: String,
    pub byline: String,
    pub text: String,
    /// Unix seconds. The sweeper's window is measured against this.
    pub at: i64,
}

/// One wave's locally readable PM projection. Linear owns the payload; sync
/// replaces this row atomically so readers never observe a partial refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmSnapshotRow {
    pub repo: String,
    pub wave: String,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub payload: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageConfig {
    Sqlite { path: PathBuf },
}

impl StorageConfig {
    pub fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }
}

#[derive(Debug)]
pub struct Store {
    sqlite: sqlite::SqliteStore,
}

async fn run_sqlite<T, F>(store: &sqlite::SqliteStore, func: F) -> StoreResult<T>
where
    T: Send + 'static,
    F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || func(store))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}

impl Store {
    pub fn wave_state(&self) -> &dyn WaveStateStore {
        self
    }

    pub fn execution(&self) -> &dyn ExecutionStore {
        self
    }

    pub fn tokens(&self) -> &dyn TokenStore {
        self
    }

    pub fn repos(&self) -> &dyn RepoStore {
        self
    }

    pub fn admin(&self) -> &dyn StoreAdmin {
        self
    }

    pub async fn put_pm_snapshot(&self, snapshot: PmSnapshotRow) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| store.put_pm_snapshot(&snapshot)).await
    }

    pub async fn pm_snapshot(
        &self,
        repo: String,
        wave: String,
    ) -> StoreResult<Option<PmSnapshotRow>> {
        run_sqlite(&self.sqlite, move |store| store.pm_snapshot(&repo, &wave)).await
    }

    // The agent bus: publish is an INSERT, subscribe is a forward poll from an
    // id cursor. No server is in the path — see `migrations/059_bus.sql`.

    /// Publish one frame, stamped now. Returns its id.
    pub async fn publish_bus(
        &self,
        channel: String,
        byline: String,
        text: String,
    ) -> StoreResult<i64> {
        let at = time::OffsetDateTime::now_utc().unix_timestamp();
        run_sqlite(&self.sqlite, move |store| {
            store.publish_bus(&channel, &byline, &text, at)
        })
        .await
    }

    /// Drop every frame published before `cutoff` (unix seconds). Production
    /// never sweeps by hand — publishing and every read carry the broom (see
    /// [`Self::swept_read`]); this aims it at a chosen cutoff so a test can
    /// age the bus out without waiting the window.
    #[cfg(test)]
    pub async fn sweep_bus(&self, cutoff: i64) -> StoreResult<usize> {
        run_sqlite(&self.sqlite, move |store| store.sweep_bus(cutoff)).await
    }

    /// Sweep the window, then read. The retention window is enforced by
    /// whoever looks, not only by whoever publishes next, so a lone report on
    /// a bus that then went quiet expires on schedule instead of waiting for a
    /// writer that may never come. Every bus read rides this broom.
    async fn swept_read<T, F>(&self, read: F) -> StoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
    {
        let cutoff = time::OffsetDateTime::now_utc().unix_timestamp() - sqlite::BUS_WINDOW_SECS;
        run_sqlite(&self.sqlite, move |store| {
            store.sweep_bus(cutoff)?;
            read(store)
        })
        .await
    }

    /// Every surviving frame after `cursor`, oldest first.
    pub async fn read_bus_after(&self, cursor: i64) -> StoreResult<Vec<BusMessage>> {
        self.swept_read(move |store| store.read_bus_after(cursor))
            .await
    }

    /// The high-water mark — where a fresh subscriber tunes in.
    pub async fn bus_head(&self) -> StoreResult<i64> {
        self.swept_read(|store| store.bus_head()).await
    }

    /// The oldest readable id. A durable cursor below `floor - 1` is a gap:
    /// frames this subscriber will never see.
    pub async fn bus_floor(&self) -> StoreResult<Option<i64>> {
        self.swept_read(|store| store.bus_floor()).await
    }

    pub async fn bus_cursor(&self, subscriber: String) -> StoreResult<Option<i64>> {
        run_sqlite(&self.sqlite, move |store| store.bus_cursor(&subscriber)).await
    }

    pub async fn set_bus_cursor(&self, subscriber: String, cursor: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.set_bus_cursor(&subscriber, cursor)
        })
        .await
    }

    pub async fn create_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_session(&session)
        })
        .await
    }

    pub async fn reserve_task_session(
        &self,
        session: &TaskSession,
        max_active: u32,
    ) -> StoreResult<bool> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_session(&session, max_active)
        })
        .await
    }

    pub async fn reserve_task_session_with_directive(
        &self,
        session: &TaskSession,
        directive: &ChildDirective,
        max_active: u32,
    ) -> StoreResult<bool> {
        let session = session.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_session_with_directive(&session, &directive, max_active)
        })
        .await
    }

    pub async fn update_task_session(&self, session: &TaskSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_task_session(&session)
        })
        .await
    }

    pub async fn reserve_task_process(
        &self,
        session: &TaskSession,
        expected_status: TaskSessionStatus,
        max_active: u32,
    ) -> StoreResult<bool> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_task_process(&session, expected_status, max_active)
        })
        .await
    }

    pub async fn get_task_session(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Option<TaskSession>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_session(&session_id)).await
    }

    pub async fn get_task_session_by_issue(&self, issue: &str) -> StoreResult<Option<TaskSession>> {
        let issue = issue.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.task_session_by_issue(&issue)
        })
        .await
    }

    pub async fn list_task_sessions(
        &self,
        wave_id: Option<&LfdId>,
    ) -> StoreResult<Vec<TaskSession>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_task_sessions(wave_id.as_ref())
        })
        .await
    }

    pub async fn create_task_command(&self, command: &TaskCommand) -> StoreResult<()> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_command(&command)
        })
        .await
    }

    pub async fn ensure_task_decision_command(
        &self,
        command: &TaskCommand,
    ) -> StoreResult<(TaskCommand, bool)> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_task_decision_command(&command)
        })
        .await
    }

    pub async fn supersede_and_create_task_command(
        &self,
        command: &TaskCommand,
    ) -> StoreResult<Vec<TaskCommandId>> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.supersede_and_insert_task_command(&command)
        })
        .await
    }

    pub async fn create_task_command_with_directive(
        &self,
        command: &TaskCommand,
        directive: &ChildDirective,
    ) -> StoreResult<Vec<TaskCommandId>> {
        let command = command.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_task_command_with_directive(&command, &directive)
        })
        .await
    }

    pub async fn get_task_command(
        &self,
        command_id: &TaskCommandId,
    ) -> StoreResult<Option<TaskCommand>> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_command(&command_id)).await
    }

    pub async fn list_task_commands(
        &self,
        session_id: &TaskSessionId,
    ) -> StoreResult<Vec<TaskCommand>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| store.task_commands(&session_id)).await
    }

    pub async fn claim_task_commands(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
    ) -> StoreResult<Vec<TaskCommand>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_task_commands(&session_id, generation)
        })
        .await
    }

    pub async fn claim_task_commands_or_stop(
        &self,
        session_id: &TaskSessionId,
        generation: u32,
        stopped_status: TaskSessionStatus,
        reason: &str,
    ) -> StoreResult<BoundaryResult> {
        let session_id = session_id.clone();
        let reason = reason.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_task_commands_or_stop(&session_id, generation, stopped_status, &reason)
        })
        .await
    }

    pub async fn accept_task_command(
        &self,
        command_id: &TaskCommandId,
        effect: Option<crate::task::TaskCommandEffect>,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.accept_task_command(&command_id, effect)
        })
        .await
    }

    pub async fn set_task_command_effect(
        &self,
        command_id: &TaskCommandId,
        effect: crate::task::TaskCommandEffect,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.set_task_command_effect(&command_id, effect)
        })
        .await
    }

    pub async fn fail_task_command(
        &self,
        command_id: &TaskCommandId,
        effect: Option<crate::task::TaskCommandEffect>,
        error: String,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_task_command(&command_id, effect, &error)
        })
        .await
    }

    pub async fn append_task_event(
        &self,
        session_id: &TaskSessionId,
        kind: &TaskEventKind,
    ) -> StoreResult<TaskEvent> {
        let session_id = session_id.clone();
        let kind = kind.clone();
        let write_session_id = session_id.clone();
        let write_kind = kind.clone();
        let event = run_sqlite(&self.sqlite, move |store| {
            store.append_task_event(&write_session_id, &write_kind)
        })
        .await?;
        if kind.is_wave_observable() {
            if let Some(session) = self.get_task_session(&session_id).await? {
                match &session.supervisor {
                    SessionSupervisor::Wave { .. } => {
                        if let Err(error) =
                            crate::lf::commands::chat::post_task_observation_to_named_wave(
                                &session.wave,
                                &session_id,
                                event.id,
                            )
                            .await
                        {
                            tracing::debug!(
                                %error,
                                %session_id,
                                event_id = event.id,
                                "live Task observation delivery failed; Wave observer will retry"
                            );
                        }
                    }
                    SessionSupervisor::Project {
                        session_id: project_session_id,
                    } => {
                        if let Err(error) =
                            crate::ops::project::wake_project_session(project_session_id).await
                        {
                            tracing::debug!(
                                %error,
                                %session_id,
                                project_session_id = %project_session_id,
                                event_id = event.id,
                                "Task observation wake failed; Project lifecycle touch will retry"
                            );
                        }
                        if let Err(error) =
                            crate::lf::commands::chat::post_task_observation_to_named_wave(
                                &session.wave,
                                &session_id,
                                event.id,
                            )
                            .await
                        {
                            tracing::debug!(
                                %error,
                                %session_id,
                                event_id = event.id,
                                "live descendant observation delivery failed; Wave observer will retry"
                            );
                        }
                    }
                }
            }
        }
        Ok(event)
    }

    pub async fn task_events_after(
        &self,
        session_id: &TaskSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_events_after(&session_id, cursor)
        })
        .await
    }

    pub async fn get_task_event(
        &self,
        session_id: &TaskSessionId,
        event_id: i64,
    ) -> StoreResult<Option<TaskEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.task_event(&session_id, event_id)
        })
        .await
    }

    pub async fn create_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_session(&session)
        })
        .await
    }

    pub async fn create_project_session_with_directive(
        &self,
        session: &ProjectSession,
        directive: &ChildDirective,
    ) -> StoreResult<()> {
        let session = session.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_session_with_directive(&session, &directive)
        })
        .await
    }

    pub async fn update_project_session(&self, session: &ProjectSession) -> StoreResult<()> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.update_project_session(&session)
        })
        .await
    }

    pub async fn reserve_project_process(
        &self,
        session: &ProjectSession,
        expected_status: ProjectSessionStatus,
    ) -> StoreResult<bool> {
        let session = session.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_project_process(&session, expected_status)
        })
        .await
    }

    pub async fn get_project_session(
        &self,
        session_id: &ProjectSessionId,
    ) -> StoreResult<Option<ProjectSession>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_session(&session_id)
        })
        .await
    }

    pub async fn get_project_session_by_project(
        &self,
        project: &str,
    ) -> StoreResult<Option<ProjectSession>> {
        let project = project.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.project_session_by_project(&project)
        })
        .await
    }

    pub async fn list_project_sessions(
        &self,
        wave_id: Option<&LfdId>,
    ) -> StoreResult<Vec<ProjectSession>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_project_sessions(wave_id.as_ref())
        })
        .await
    }

    pub async fn create_project_command(&self, command: &ProjectCommand) -> StoreResult<()> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_command(&command)
        })
        .await
    }

    pub async fn ensure_project_decision_command(
        &self,
        command: &ProjectCommand,
    ) -> StoreResult<(ProjectCommand, bool)> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_project_decision_command(&command)
        })
        .await
    }

    pub async fn supersede_and_create_project_command(
        &self,
        command: &ProjectCommand,
    ) -> StoreResult<Vec<ProjectCommandId>> {
        let command = command.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.supersede_and_insert_project_command(&command)
        })
        .await
    }

    pub async fn create_project_command_with_directive(
        &self,
        command: &ProjectCommand,
        directive: &ChildDirective,
    ) -> StoreResult<Vec<ProjectCommandId>> {
        let command = command.clone();
        let directive = directive.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.insert_project_command_with_directive(&command, &directive)
        })
        .await
    }

    pub async fn get_project_command(
        &self,
        command_id: &ProjectCommandId,
    ) -> StoreResult<Option<ProjectCommand>> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_command(&command_id)
        })
        .await
    }

    pub async fn claim_project_commands(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
    ) -> StoreResult<Vec<ProjectCommand>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_project_commands(&session_id, generation)
        })
        .await
    }

    pub async fn claim_project_commands_or_stop(
        &self,
        session_id: &ProjectSessionId,
        generation: u32,
        stopped_status: ProjectSessionStatus,
        reason: String,
    ) -> StoreResult<(Vec<ProjectCommand>, Option<ProjectSession>)> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.claim_project_commands_or_stop(&session_id, generation, stopped_status, &reason)
        })
        .await
    }

    pub async fn accept_project_command(
        &self,
        command_id: &ProjectCommandId,
        effect: Option<crate::task::TaskCommandEffect>,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.accept_project_command(&command_id, effect)
        })
        .await
    }

    pub async fn fail_project_command(
        &self,
        command_id: &ProjectCommandId,
        effect: Option<crate::task::TaskCommandEffect>,
        error: String,
    ) -> StoreResult<()> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.fail_project_command(&command_id, effect, &error)
        })
        .await
    }

    pub async fn append_project_event(
        &self,
        session_id: &ProjectSessionId,
        kind: &ProjectEventKind,
    ) -> StoreResult<ProjectEvent> {
        let session_id = session_id.clone();
        let kind = kind.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.append_project_event(&session_id, &kind)
        })
        .await
    }

    pub async fn project_events_after(
        &self,
        session_id: &ProjectSessionId,
        cursor: i64,
    ) -> StoreResult<Vec<ProjectEvent>> {
        let session_id = session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.project_events_after(&session_id, cursor)
        })
        .await
    }

    pub async fn pending_observations(
        &self,
        supervisor: &SessionSupervisor,
    ) -> StoreResult<Vec<ObservationOutboxRow>> {
        let supervisor = supervisor.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_observations(&supervisor)
        })
        .await
    }

    pub async fn mark_observation_delivered(&self, id: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.mark_observation_delivered(id)
        })
        .await
    }

    pub async fn consume_task_observation_for_project(
        &self,
        project_session_id: &ProjectSessionId,
        observation: &ObservationOutboxRow,
    ) -> StoreResult<bool> {
        let project_session_id = project_session_id.clone();
        let observation = observation.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.consume_task_observation_for_project(&project_session_id, &observation)
        })
        .await
    }

    pub async fn child_directives(&self, target: &ChildRef) -> StoreResult<Vec<ChildDirective>> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.child_directives(&target)).await
    }

    pub async fn child_directive_for_command(
        &self,
        command_id: &TaskCommandId,
    ) -> StoreResult<Option<ChildDirective>> {
        let command_id = command_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.child_directive_for_command(&command_id)
        })
        .await
    }

    pub async fn mark_child_directive_applied(
        &self,
        target: &ChildRef,
        version: u32,
    ) -> StoreResult<()> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.mark_child_directive_applied(&target, version)
        })
        .await
    }

    pub async fn incorporate_child_directive(
        &self,
        target: &ChildRef,
        version: u32,
        summary: &str,
    ) -> StoreResult<(ChildDirective, bool)> {
        let target = target.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.incorporate_child_directive(&target, version, &summary)
        })
        .await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        WaveStateStore::list_waves(self, repo).await
    }

    /// A chord's contents: the waves whose `parent_wave_id` is `parent`,
    /// ordered by creation.
    pub async fn list_child_waves(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        WaveStateStore::children_of(self, parent).await
    }

    pub async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave(self, wave_id).await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave_by_name(self, name).await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::create_wave(self, wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::update_wave(self, wave).await
    }

    pub async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_wave(self, wave_id).await
    }

    pub async fn list_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Run>> {
        WaveStateStore::list_runs(self, wave_id, limit).await
    }

    pub async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>> {
        WaveStateStore::list_runs_active_or_ended_since(self, ended_since).await
    }

    pub async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_run(self, run_id).await
    }

    pub async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_active_run(self, wave_id).await
    }

    pub async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::count_active_runs(self, wave_id).await
    }

    pub async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_latest_run(self, wave_id).await
    }

    pub async fn create_run(&self, run: &Run) -> StoreResult<()> {
        WaveStateStore::create_run(self, run).await
    }

    pub async fn update_run(&self, run: &Run) -> StoreResult<()> {
        WaveStateStore::update_run(self, run).await
    }

    pub async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        WaveStateStore::get_live_pr_state(self, repo_id, pr_number).await
    }

    pub async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        WaveStateStore::upsert_live_pr_state(self, state).await
    }

    pub async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        WaveStateStore::list_attention_items(self, status, kind).await
    }

    pub async fn get_attention_item(
        &self,
        attention_id: &LfdId,
    ) -> StoreResult<Option<AttentionItem>> {
        WaveStateStore::get_attention_item(self, attention_id).await
    }

    pub async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        WaveStateStore::find_attention_item_for_run(self, run_id, kind).await
    }

    pub async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        WaveStateStore::upsert_attention_item(self, item).await
    }

    pub async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_attention_item(self, attention_id).await
    }

    pub async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        WaveStateStore::get_summary(self, wave_id).await
    }

    pub async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        WaveStateStore::upsert_summary(self, summary).await
    }

    pub async fn list_chat_memory_blocks(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ChatMemoryBlock>> {
        WaveStateStore::list_chat_memory_blocks(self, wave_id).await
    }

    pub async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        WaveStateStore::upsert_chat_memory_block(self, block).await
    }

    pub async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        WaveStateStore::delete_chat_memory_block(self, wave_id, name).await
    }

    pub async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        WaveStateStore::list_chat_messages(self, wave_id).await
    }

    pub async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        WaveStateStore::create_chat_message(self, message).await
    }

    pub async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        RepoStore::list_repos(self).await
    }

    pub async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        RepoStore::get_repo(self, path).await
    }

    pub async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        RepoStore::upsert_repo(self, repo).await
    }

    pub async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        RepoStore::delete_repo(self, path).await
    }

    pub async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        RepoStore::get_repo_by_repo_id(self, repo_id).await
    }

    pub async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        RepoStore::list_edges(self).await
    }

    pub async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        RepoStore::add_edge(self, edge).await
    }

    pub async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        RepoStore::remove_edge(self, parent_id, child_id).await
    }

    pub async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        RepoStore::children(self, repo_id).await
    }

    pub async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        RepoStore::parents(self, repo_id).await
    }

    pub async fn list_fork_runs(
        &self,
        run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        ExecutionStore::list_fork_runs(self, run_id, step_index).await
    }

    pub async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        ExecutionStore::upsert_fork_run(self, fork_run).await
    }

    pub async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        ExecutionStore::delete_fork_runs(self, run_id, step_index).await
    }

    pub async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        ExecutionStore::fail_orphaned_runs(self).await
    }

    pub async fn create_control_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::create_session(self, session).await
    }

    pub async fn get_control_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        ControlSessionStore::get_session(self, session_id).await
    }

    pub async fn list_control_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        ControlSessionStore::list_sessions(self, wave_id, statuses).await
    }

    /// This wave's live sessions plus sessions completed at or after
    /// `completed_since` (unix seconds) — a poller's working set, bounded
    /// regardless of how much terminal history the wave accumulates.
    pub async fn list_recent_control_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>> {
        ControlSessionStore::list_recent_sessions(self, wave_id, completed_since).await
    }

    /// The wave's live brain, if any: a non-terminal `WaveAgent` session —
    /// either lfd-launched (`POST /waves/{id}/run`) or a self-registered
    /// `lf serve` listener. One-brain enforcement (run_wave idempotency, the
    /// loop-ticker skip, wave-server registration conflicts) keys on this
    /// single fact.
    pub async fn live_wave_agent_session(&self, wave_id: &LfdId) -> StoreResult<Option<Session>> {
        let sessions = self
            .list_control_sessions(
                Some(wave_id),
                Some(crate::lfd::types::LIVE_SESSION_STATUSES),
            )
            .await?;
        Ok(sessions
            .into_iter()
            .find(|session| session.session_use == crate::lfd::types::SessionUse::WaveAgent))
    }

    /// Record a session in the run registry. The db IS the registry: `lf`
    /// writes its own row here directly — self-registered flow runs, the
    /// `lf serve` listener's WaveAgent row, placed `lf` runs — no
    /// daemon in the path. The writer later marks the row terminal via
    /// [`Store::update_control_session`].
    pub async fn register_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::create_session(self, session).await
    }

    /// Live sessions grouped under one worktree, keyed by the worktree
    /// directory's basename (`<repo>.<wave>`, `<repo>.<wave>.<id>`, ...):
    /// everything currently running in that tree, whoever launched it.
    pub async fn active_sessions_by_worktree(
        &self,
        worktree_name: &str,
    ) -> StoreResult<Vec<Session>> {
        let sessions = self
            .list_control_sessions(None, Some(crate::lfd::types::LIVE_SESSION_STATUSES))
            .await?;
        Ok(sessions
            .into_iter()
            .filter(|session| {
                std::path::Path::new(&session.cwd)
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some(worktree_name)
            })
            .collect())
    }

    pub async fn get_active_control_session_for_run(
        &self,
        run_id: &LfdId,
    ) -> StoreResult<Option<Session>> {
        ControlSessionStore::get_active_session_for_run(self, run_id).await
    }

    pub async fn update_control_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::update_session(self, session).await
    }

    pub async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        TokenStore::get_provider_token(self, provider).await
    }

    pub async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        TokenStore::upsert_provider_token(self, token).await
    }

    pub async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        TokenStore::delete_provider_token(self, provider).await
    }

    pub async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        TokenStore::list_provider_tokens(self).await
    }

    pub async fn health_check(&self) -> StoreResult<()> {
        StoreAdmin::health_check(self).await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        StoreAdmin::schema_version(self).await
    }
}

#[async_trait::async_trait]
pub trait WaveStateStore: Send + Sync {
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>>;
    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    async fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    async fn list_runs(&self, wave_id: Option<&LfdId>, limit: Option<u32>)
        -> StoreResult<Vec<Run>>;
    /// Non-terminal runs plus runs that ended at or after `ended_since` —
    /// the push bridge's working set, so it never scans the whole terminal
    /// history.
    async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>>;
    async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32>;
    async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn create_run(&self, run: &Run) -> StoreResult<()>;
    async fn update_run(&self, run: &Run) -> StoreResult<()>;
    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>>;
    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()>;
    async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>>;
    async fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>>;
    async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>>;
    async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()>;
    async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32>;
    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>>;
    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()>;
    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()>;

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>>;
    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()>;
}

#[async_trait::async_trait]
pub trait RepoStore: Send + Sync {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>>;
    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>>;
    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()>;
    async fn delete_repo(&self, path: &str) -> StoreResult<()>;
    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>>;
    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>>;
    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()>;
    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()>;
    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
}

#[async_trait::async_trait]
pub trait ExecutionStore: Send + Sync {
    async fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>>;
    async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>>;
    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32>;

    async fn fail_orphaned_runs(&self) -> StoreResult<u32>;
}

#[async_trait::async_trait]
pub trait ControlSessionStore: Send + Sync {
    async fn create_session(&self, session: &Session) -> StoreResult<()>;
    async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>>;
    async fn list_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>>;
    async fn list_recent_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>>;
    async fn get_active_session_for_run(&self, run_id: &LfdId) -> StoreResult<Option<Session>>;
    async fn update_session(&self, session: &Session) -> StoreResult<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth,
    ApiKey,
}

impl CredentialType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "apikey",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "apikey" => Self::ApiKey,
            _ => Self::OAuth,
        }
    }
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub oauth_client_id: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

#[async_trait::async_trait]
pub trait TokenStore: Send + Sync {
    async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>>;
    async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()>;
    async fn delete_provider_token(&self, provider: &str) -> StoreResult<()>;
    async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>>;
}

#[async_trait::async_trait]
pub trait StoreAdmin: Send + Sync {
    async fn health_check(&self) -> StoreResult<()>;
    async fn schema_version(&self) -> StoreResult<String>;
}

#[async_trait::async_trait]
impl WaveStateStore for Store {
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let waves = {
            let repo = repo.map(str::to_string);
            run_sqlite(&self.sqlite, move |store| store.list_waves(repo.as_deref())).await
        }?;
        Ok(waves)
    }

    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.list_child_waves(&parent)).await
    }

    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.get_wave(&wave_id)).await
    }

    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        run_sqlite(&self.sqlite, move |store| store.get_wave_by_name(&name)).await
    }

    async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        {
            let wave = wave.clone();
            run_sqlite(&self.sqlite, move |store| store.create_wave(&wave)).await
        }
    }

    async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        {
            let wave = wave.clone();
            run_sqlite(&self.sqlite, move |store| store.update_wave(&wave)).await
        }
    }

    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.delete_wave(&wave_id)).await
        }
    }

    async fn list_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Run>> {
        {
            let wave_id = wave_id.cloned();
            run_sqlite(&self.sqlite, move |store| {
                store.list_runs(wave_id.as_ref(), limit)
            })
            .await
        }
    }

    async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>> {
        {
            run_sqlite(&self.sqlite, move |store| {
                store.list_runs_active_or_ended_since(ended_since)
            })
            .await
        }
    }

    async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_run(&run_id)).await
        }
    }

    async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_active_run(&wave_id)).await
        }
    }

    async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.count_active_runs(&wave_id)).await
        }
    }

    async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_latest_run(&wave_id)).await
        }
    }

    async fn create_run(&self, run: &Run) -> StoreResult<()> {
        {
            let run = run.clone();
            run_sqlite(&self.sqlite, move |store| store.create_run(&run)).await
        }
    }

    async fn update_run(&self, run: &Run) -> StoreResult<()> {
        {
            let run = run.clone();
            run_sqlite(&self.sqlite, move |store| store.update_run(&run)).await
        }
    }

    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        {
            let repo_id = repo_id.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.get_live_pr_state(&repo_id, pr_number)
            })
            .await
        }
    }

    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        {
            let state = state.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_live_pr_state(&state)
            })
            .await
        }
    }

    async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        {
            run_sqlite(&self.sqlite, move |store| {
                store.list_attention_items(status, kind)
            })
            .await
        }
    }

    async fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>> {
        {
            let attention_id = attention_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_attention_item(&attention_id)
            })
            .await
        }
    }

    async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.find_attention_item_for_run(&run_id, kind)
            })
            .await
        }
    }

    async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        {
            let item = item.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_attention_item(&item)
            })
            .await
        }
    }

    async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        {
            let attention_id = attention_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_attention_item(&attention_id)
            })
            .await
        }
    }

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_summary(&wave_id)).await
        }
    }

    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        {
            let summary = summary.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_summary(&summary)).await
        }
    }

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_chat_memory_blocks(&wave_id)
            })
            .await
        }
    }

    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        {
            let block = block.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_chat_memory_block(&block)
            })
            .await
        }
    }

    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        {
            let wave_id = wave_id.clone();
            let name = name.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_chat_memory_block(&wave_id, &name)
            })
            .await
        }
    }

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_chat_messages(&wave_id)
            })
            .await
        }
    }

    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        {
            let message = message.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.create_chat_message(&message)
            })
            .await
        }
    }
}

#[async_trait::async_trait]
impl RepoStore for Store {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_repos()).await
        }
    }

    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        {
            let path = path.to_string();
            run_sqlite(&self.sqlite, move |store| store.get_repo(&path)).await
        }
    }

    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        {
            let repo = repo.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_repo(&repo)).await
        }
    }

    async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        {
            let path = path.to_string();
            run_sqlite(&self.sqlite, move |store| store.delete_repo(&path)).await
        }
    }

    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_repo_by_repo_id(&repo_id)
            })
            .await
        }
    }

    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_edges()).await
        }
    }

    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        {
            let edge = edge.clone();
            run_sqlite(&self.sqlite, move |store| store.add_edge(&edge)).await
        }
    }

    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        {
            let parent_id = parent_id.clone();
            let child_id = child_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.remove_edge(&parent_id, &child_id)
            })
            .await
        }
    }

    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| store.children(&repo_id)).await
        }
    }

    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| store.parents(&repo_id)).await
        }
    }
}

#[async_trait::async_trait]
impl ExecutionStore for Store {
    async fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_fork_runs(&run_id, step_index)
            })
            .await
        }
    }

    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        {
            let fork_run = fork_run.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_fork_run(&fork_run)).await
        }
    }

    async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_orphaned_fork_runs()).await
        }
    }

    async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_fork_runs(&run_id, step_index)
            })
            .await
        }
    }

    async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        {
            run_sqlite(&self.sqlite, |store| store.fail_orphaned_runs()).await
        }
    }
}

#[async_trait::async_trait]
impl ControlSessionStore for Store {
    async fn create_session(&self, session: &Session) -> StoreResult<()> {
        {
            let session = session.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.create_control_session(&session)
            })
            .await
        }
    }

    async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        {
            let session_id = session_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_control_session(&session_id)
            })
            .await
        }
    }

    async fn list_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        {
            let wave_id = wave_id.cloned();
            let statuses = statuses.map(|values| values.to_vec());
            run_sqlite(&self.sqlite, move |store| {
                store.list_control_sessions(wave_id.as_ref(), statuses.as_deref())
            })
            .await
        }
    }

    async fn list_recent_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_recent_control_sessions(&wave_id, completed_since)
            })
            .await
        }
    }

    async fn get_active_session_for_run(&self, run_id: &LfdId) -> StoreResult<Option<Session>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_active_control_session_for_run(&run_id)
            })
            .await
        }
    }

    async fn update_session(&self, session: &Session) -> StoreResult<()> {
        {
            let session = session.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.update_control_session(&session)
            })
            .await
        }
    }
}

#[async_trait::async_trait]
impl TokenStore for Store {
    async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        {
            let provider = provider.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.get_provider_token(&provider)
            })
            .await
        }
    }

    async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        {
            let token = token.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_provider_token(&token)
            })
            .await
        }
    }

    async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        {
            let provider = provider.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_provider_token(&provider)
            })
            .await
        }
    }

    async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_provider_tokens()).await
        }
    }
}

#[async_trait::async_trait]
impl StoreAdmin for Store {
    async fn health_check(&self) -> StoreResult<()> {
        {
            run_sqlite(&self.sqlite, |store| store.health_check()).await
        }
    }

    async fn schema_version(&self) -> StoreResult<String> {
        {
            run_sqlite(&self.sqlite, |store| store.schema_version()).await
        }
    }
}

pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store> {
    let StorageConfig::Sqlite { path } = cfg;
    Ok(Store {
        sqlite: sqlite::SqliteStore::new(path)?,
    })
}

/// Open the machine's shared registry store only if one already exists.
/// `None` means this machine has no registry yet; callers that instrument best-effort (lf
/// self-registration, the wave server) treat that as "not instrumented" and
/// stay silent rather than conjuring an empty db.
pub async fn open_existing_store() -> Option<Store> {
    let cfg = crate::lfd::storage_config_from_env().ok()?;
    let StorageConfig::Sqlite { path } = &cfg;
    if !path.exists() {
        return None;
    }
    // lf-direct openers can meet a db created by an older lfd (the daemon
    // migrates only at its own boot). Apply pending migrations here so a
    // direct writer never hits schema drift; versioned migrations make a
    // concurrent second applier a no-op, and sqlite locking serializes
    // them. A failed migration means the db is unusable for us: warn and
    // report "not instrumented" rather than limping on a wrong schema.
    let conn = rusqlite::Connection::open(path).ok()?;
    if let Err(err) = migrations::apply_sqlite(&conn) {
        tracing::warn!(?path, %err, "registry store migration failed; running uninstrumented");
        return None;
    }
    open_store(&cfg).await.ok()
}

pub async fn migrate_store(cfg: &StorageConfig, status_only: bool) -> StoreResult<String> {
    let StorageConfig::Sqlite { path } = cfg;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| StoreError::InvalidData(format!("failed to create db dir: {err}")))?;
    }
    let conn = rusqlite::Connection::open(path)?;
    if !status_only {
        migrations::apply_sqlite(&conn)?;
    }
    migrations::latest_version_sqlite(&conn)
}
pub type SharedStore = Arc<Store>;

#[cfg(test)]
mod tests {
    use super::sqlite::SqliteStore;
    use super::{
        open_store, ExecutionStore, ForkRun, ForkRunStatus, PmSnapshotRow, RunEventRow,
        StorageConfig,
    };
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        ChatMemoryBlock, Repo, RepoEdge, RepoId, Run, RunStatus, Summary, Wave, WaveStatus,
        DEFAULT_WAVE_FLOW,
    };
    use crate::project_session::{
        ChildEventPayload, ChildSessionRef, ProjectCommand, ProjectCommandKind,
        ProjectCommandSource, ProjectDecisionId, ProjectProcess, ProjectSession, ProjectSessionId,
        ProjectSessionStatus, SessionSupervisor,
    };
    use crate::task::{
        BoundaryResult, ChildDirective, ChildRef, LinearIssueId, LinearIssueRef, LinearProjectId,
        LinearProjectRef, PmWritebackOperation, PmWritebackState, TaskCommand, TaskCommandEffect,
        TaskCommandKind, TaskCommandSource, TaskCommandState, TaskDecisionId, TaskEventKind,
        TaskSession, TaskSessionId, TaskSessionStatus,
    };
    use std::env;
    use std::path::PathBuf;
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: vec!["focus".to_string()],
            area: vec!["src".to_string()],
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_run(wave: &Wave, status: RunStatus) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: wave.repo().to_string(),
            flow: DEFAULT_WAVE_FLOW.to_string(),
            task: None,
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            iteration: 0,
            step_index: 0,
            status,
            worktree: "/repo".to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            repair_of: None,
            pr: None,
        }
    }

    fn make_task_session(wave: &Wave) -> TaskSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        TaskSession {
            id: TaskSessionId::new(),
            issue: LinearIssueRef {
                id: LinearIssueId::new("issue-uuid").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Add hello world".to_string(),
                description: "Ship one command".to_string(),
            },
            project: LinearProjectRef {
                id: LinearProjectId::new("project-uuid").unwrap(),
                slug: "developer-efficiency".to_string(),
                name: "Developer Efficiency".to_string(),
                context: "Definition:\nKeep local work fast.".to_string(),
            },
            pm_snapshot_synced_at: now.unix_timestamp(),
            pm_snapshot_warning: None,
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            wave: wave.name().to_string(),
            supervisor: crate::project_session::SessionSupervisor::Wave {
                wave_id: wave.id().clone(),
            },
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Created,
            status_reason: "task session reserved".to_string(),
            status_at: now,
            worktree: PathBuf::from("/repo.inf-123"),
            branch: "jack/inf-123".to_string(),
            base_commit: "deadbeef".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            process: None,
            pull_request: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_project_session(wave: &Wave) -> ProjectSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        ProjectSession {
            id: ProjectSessionId::new(),
            project: LinearProjectRef {
                id: LinearProjectId::new("project-uuid").unwrap(),
                slug: "developer-efficiency".to_string(),
                name: "Developer Efficiency".to_string(),
                context: "Definition:\nKeep local work fast.".to_string(),
            },
            wave_id: wave.id().clone(),
            wave: wave.name().to_string(),
            repo: wave.repo().to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Running,
            status_reason: "project turn active".to_string(),
            status_at: now,
            iteration: 1,
            task_event_cursor: 0,
            state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-project".to_string()),
            process: Some(ProjectProcess {
                generation: 1,
                pid: None,
                tmux_name: "lf-project-test".to_string(),
                started_at: now,
            }),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn child_directives_reserve_replace_and_incorporate_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut task = make_task_session(&wave);
        task.current_directive_version = 1;
        let task_target = ChildRef::Task(task.id.clone());
        let task_initial = ChildDirective::initial(
            task_target.clone(),
            "Fix the parser before the docs".to_string(),
            TaskCommandSource::Wave(wave.id().clone()),
        );
        assert!(store
            .reserve_task_session_with_directive(&task, &task_initial, 2)
            .await
            .unwrap());
        assert_eq!(store.child_directives(&task_target).await.unwrap().len(), 1);
        let stale_task = task.clone();

        let task_command = TaskCommand::new(
            task.id.clone(),
            TaskCommandSource::Wave(wave.id().clone()),
            TaskCommandKind::Steer {
                text: "Fix the parser before the docs or tests".to_string(),
            },
        );
        let task_replacement = ChildDirective::replacement(
            task_target.clone(),
            2,
            "Fix the parser before the docs or tests".to_string(),
            task_command.source.clone(),
            task_command.id.clone(),
        );
        store
            .create_task_command_with_directive(&task_command, &task_replacement)
            .await
            .unwrap();
        store
            .mark_child_directive_applied(&task_target, 2)
            .await
            .unwrap();
        store
            .incorporate_child_directive(&task_target, 2, "Parser remains first")
            .await
            .unwrap();
        store.update_task_session(&stale_task).await.unwrap();
        let persisted_task = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted_task.current_directive_version, 2);
        assert_eq!(persisted_task.incorporated_directive_version, 2);

        let mut project = make_project_session(&wave);
        project.current_directive_version = 1;
        let project_target = ChildRef::Project(project.id.clone());
        let project_initial = ChildDirective::initial(
            project_target.clone(),
            "Pursue onboarding first".to_string(),
            TaskCommandSource::Wave(wave.id().clone()),
        );
        store
            .create_project_session_with_directive(&project, &project_initial)
            .await
            .unwrap();
        let stale_project = project.clone();

        let command = ProjectCommand::new(
            project.id.clone(),
            ProjectCommandSource::Wave(wave.id().clone()),
            ProjectCommandKind::Steer {
                text: "Prove the parser path first".to_string(),
            },
        );
        let replacement = ChildDirective::replacement(
            project_target.clone(),
            2,
            "Prove the parser path first".to_string(),
            command.source.clone(),
            command.id.clone(),
        );
        store
            .create_project_command_with_directive(&command, &replacement)
            .await
            .unwrap();
        let persisted = store
            .get_project_session(&project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.current_directive_version, 2);
        assert_eq!(
            store.child_directives(&project_target).await.unwrap().len(),
            2
        );
        assert!(store
            .incorporate_child_directive(&project_target, 1, "stale")
            .await
            .is_err());
        store
            .mark_child_directive_applied(&project_target, 2)
            .await
            .unwrap();
        assert!(
            store
                .incorporate_child_directive(&project_target, 2, "Parser is now first")
                .await
                .unwrap()
                .1
        );
        assert!(
            !store
                .incorporate_child_directive(&project_target, 2, "Parser is now first")
                .await
                .unwrap()
                .1
        );
        store.update_project_session(&stale_project).await.unwrap();
        assert_eq!(
            store
                .get_project_session(&project.id)
                .await
                .unwrap()
                .unwrap()
                .incorporated_directive_version,
            2
        );
    }

    #[tokio::test]
    async fn project_sessions_persist_commands_and_receive_task_observations() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let command = ProjectCommand::new(
            project.id.clone(),
            ProjectCommandSource::Wave(wave.id().clone()),
            ProjectCommandKind::FollowUp {
                text: "audit the parser".to_string(),
            },
        );
        store.create_project_command(&command).await.unwrap();
        let claimed = store.claim_project_commands(&project.id, 1).await.unwrap();
        assert_eq!(claimed.len(), 1);
        store
            .accept_project_command(&command.id, Some(TaskCommandEffect::NextTurn))
            .await
            .unwrap();
        assert_eq!(
            store
                .get_project_command(&command.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            TaskCommandState::Accepted
        );
        assert!(store.get_task_command(&command.id).await.unwrap().is_none());
        let decision = ProjectDecisionId::new();
        let decision_command = ProjectCommand::new(
            project.id.clone(),
            ProjectCommandSource::Wave(wave.id().clone()),
            ProjectCommandKind::Decide {
                decision_id: decision,
                choice: "approve".to_string(),
                message: None,
            },
        );
        assert!(
            store
                .ensure_project_decision_command(&decision_command)
                .await
                .unwrap()
                .1
        );
        assert!(
            !store
                .ensure_project_decision_command(&decision_command)
                .await
                .unwrap()
                .1
        );

        let mut task = make_task_session(&wave);
        task.supervisor = SessionSupervisor::Project {
            session_id: project.id.clone(),
        };
        store.create_task_session(&task).await.unwrap();
        store
            .sqlite
            .append_task_event(
                &task.id,
                &TaskEventKind::Failed {
                    error: "provider stopped".to_string(),
                    resumable: true,
                },
            )
            .unwrap();
        let observations = store
            .pending_observations(&SessionSupervisor::Project {
                session_id: project.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(observations.len(), 1);
        assert!(matches!(
            (&observations[0].source, &observations[0].payload),
            (
                ChildSessionRef::Task { session_id },
                ChildEventPayload::Task { event: TaskEventKind::Failed { .. } }
            ) if session_id == &task.id
        ));
        let wave_observations = store
            .pending_observations(&SessionSupervisor::Wave {
                wave_id: wave.id().clone(),
            })
            .await
            .unwrap();
        assert!(wave_observations.iter().any(|observation| matches!(
            (&observation.source, &observation.payload),
            (
                ChildSessionRef::Task { session_id },
                ChildEventPayload::Task { event: TaskEventKind::Failed { .. } }
            ) if session_id == &task.id
        )));
        assert!(store
            .consume_task_observation_for_project(&project.id, &observations[0])
            .await
            .unwrap());
        assert!(!store
            .consume_task_observation_for_project(&project.id, &observations[0])
            .await
            .unwrap());
        let project_events = store.project_events_after(&project.id, 0).await.unwrap();
        assert_eq!(
            project_events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::project_session::ProjectEventKind::TaskObserved { .. }
                ))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn task_session_commands_reclaim_by_generation_and_events_are_durable() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let mut session = make_task_session(&wave);
        session.pm_snapshot_warning = Some("using cached snapshot".to_string());
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: "Linear unavailable".to_string(),
        };
        store.create_task_session(&session).await.unwrap();

        let loaded = store
            .get_task_session_by_issue("INF-123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded, session);

        let command = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Steer {
                text: "rename the flag".to_string(),
            },
        );
        store.create_task_command(&command).await.unwrap();
        let first_claim = store.claim_task_commands(&session.id, 1).await.unwrap();
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, command.id);
        assert_eq!(first_claim[0].kind, command.kind);
        assert_eq!(first_claim[0].state, TaskCommandState::Claimed);
        assert_eq!(first_claim[0].claimed_by_generation, Some(1));
        assert_eq!(
            store.claim_task_commands(&session.id, 2).await.unwrap()[0].claimed_by_generation,
            Some(2)
        );
        store
            .accept_task_command(&command.id, Some(TaskCommandEffect::LiveSteer))
            .await
            .unwrap();
        let accepted = store.get_task_command(&command.id).await.unwrap().unwrap();
        assert_eq!(accepted.state, TaskCommandState::Accepted);
        assert_eq!(accepted.effect, Some(TaskCommandEffect::LiveSteer));
        assert!(store
            .claim_task_commands(&session.id, 3)
            .await
            .unwrap()
            .is_empty());

        let follow_up_a = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::FollowUp {
                text: "A".to_string(),
            },
        );
        let follow_up_b = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::FollowUp {
                text: "B".to_string(),
            },
        );
        store.create_task_command(&follow_up_a).await.unwrap();
        store.create_task_command(&follow_up_b).await.unwrap();
        let interrupt = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Interrupt {
                replacement: Some("C".to_string()),
            },
        );
        let superseded = store
            .supersede_and_create_task_command(&interrupt)
            .await
            .unwrap();
        assert_eq!(
            superseded,
            vec![follow_up_a.id.clone(), follow_up_b.id.clone()]
        );
        assert_eq!(
            store
                .get_task_command(&follow_up_a.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            TaskCommandState::Superseded
        );
        assert_eq!(
            store
                .claim_task_commands(&session.id, 3)
                .await
                .unwrap()
                .into_iter()
                .map(|command| command.id)
                .collect::<Vec<_>>(),
            vec![interrupt.id.clone()]
        );
        store
            .fail_task_command(
                &interrupt.id,
                Some(TaskCommandEffect::Replacement),
                "provider control failed".to_string(),
            )
            .await
            .unwrap();
        let failed = store
            .get_task_command(&interrupt.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.state, TaskCommandState::Failed);
        assert_eq!(failed.effect, Some(TaskCommandEffect::Replacement));
        assert_eq!(failed.error.as_deref(), Some("provider control failed"));
        assert!(store
            .claim_task_commands(&session.id, 4)
            .await
            .unwrap()
            .is_empty());

        let event = store
            .append_task_event(
                &session.id,
                &TaskEventKind::Progress {
                    summary: "tests pass".to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            store.task_events_after(&session.id, 0).await.unwrap(),
            vec![event]
        );

        let mut second = make_task_session(&wave);
        second.issue.id = LinearIssueId::new("issue-two").unwrap();
        second.issue.identifier = "INF-124".to_string();
        second.worktree = PathBuf::from("/repo.inf-124");
        second.branch = "jack/inf-124".to_string();
        assert!(!store.reserve_task_session(&second, 1).await.unwrap());
        assert!(store
            .get_task_session_by_issue("INF-124")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn task_boundary_atomically_claims_work_or_stops_the_generation() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut with_command = make_task_session(&wave);
        with_command.begin_generation("task-a".to_string());
        with_command.set_status(TaskSessionStatus::Running, "provider active");
        store.create_task_session(&with_command).await.unwrap();
        let command = TaskCommand::new(
            with_command.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::FollowUp {
                text: "arrived at the boundary".to_string(),
            },
        );
        store.create_task_command(&command).await.unwrap();

        let claimed = store
            .claim_task_commands_or_stop(
                &with_command.id,
                1,
                TaskSessionStatus::Waiting,
                "turn complete",
            )
            .await
            .unwrap();
        let BoundaryResult::Commands(commands) = claimed else {
            panic!("boundary stopped despite a persisted command");
        };
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, command.id);
        assert_eq!(
            store
                .get_task_session(&with_command.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskSessionStatus::Running
        );

        let mut without_command = make_task_session(&wave);
        without_command.issue.id = LinearIssueId::new("other-issue").unwrap();
        without_command.issue.identifier = "INF-124".to_string();
        without_command.worktree = PathBuf::from("/repo.inf-124");
        without_command.branch = "jack/inf-124".to_string();
        without_command.begin_generation("task-b".to_string());
        without_command.set_status(TaskSessionStatus::Running, "provider active");
        store.create_task_session(&without_command).await.unwrap();
        let stopped = store
            .claim_task_commands_or_stop(
                &without_command.id,
                1,
                TaskSessionStatus::Waiting,
                "turn complete",
            )
            .await
            .unwrap();
        let BoundaryResult::Stopped(stopped) = stopped else {
            panic!("empty boundary did not stop");
        };
        assert_eq!(stopped.status, TaskSessionStatus::Waiting);
        assert_eq!(stopped.status_reason, "turn complete");
    }

    #[tokio::test]
    async fn duplicate_task_decision_reuses_one_durable_command() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let session = make_task_session(&wave);
        store.create_task_session(&session).await.unwrap();
        let decision_id = TaskDecisionId::new();
        let first = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Decide {
                decision_id: decision_id.clone(),
                choice: "revise".to_string(),
                message: Some("cover the boundary".to_string()),
            },
        );
        let duplicate = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Decide {
                decision_id,
                choice: "approve".to_string(),
                message: None,
            },
        );

        let (stored, created) = store.ensure_task_decision_command(&first).await.unwrap();
        assert!(created);
        assert_eq!(stored.id, first.id);
        let (stored, created) = store
            .ensure_task_decision_command(&duplicate)
            .await
            .unwrap();
        assert!(!created);
        assert_eq!(stored.id, first.id);
        assert_eq!(
            store.list_task_commands(&session.id).await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn task_process_resume_reserves_wave_capacity_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut session = make_task_session(&wave);
        session.set_status(TaskSessionStatus::Waiting, "waiting for review");
        store.create_task_session(&session).await.unwrap();

        let mut first_resume = session.clone();
        assert_eq!(first_resume.begin_generation("task-one".to_string()), 1);
        assert!(store
            .reserve_task_process(&first_resume, TaskSessionStatus::Waiting, 1)
            .await
            .unwrap());

        let mut racing_resume = session.clone();
        racing_resume.begin_generation("task-one".to_string());
        assert!(!store
            .reserve_task_process(&racing_resume, TaskSessionStatus::Waiting, 1)
            .await
            .unwrap());

        let mut second = make_task_session(&wave);
        second.issue.id = LinearIssueId::new("issue-two").unwrap();
        second.issue.identifier = "INF-124".to_string();
        second.worktree = PathBuf::from("/repo.inf-124");
        second.branch = "jack/inf-124".to_string();
        second.set_status(TaskSessionStatus::Waiting, "waiting for capacity");
        store.create_task_session(&second).await.unwrap();
        second.begin_generation("task-two".to_string());
        assert!(!store
            .reserve_task_process(&second, TaskSessionStatus::Waiting, 1)
            .await
            .unwrap());

        let loaded = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, TaskSessionStatus::Starting);
        assert_eq!(loaded.process.unwrap().generation, 1);
    }

    async fn run_store_basic_suite(store: &super::Store) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(wave.id()).await.expect("get wave").is_some());

        wave.set_status(WaveStatus::Paused);
        store.update_wave(&wave).await.expect("update wave");
        let loaded = store
            .get_wave(wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(loaded.status(), WaveStatus::Paused);
        assert_eq!(loaded.repo(), "/repo");
        assert_eq!(loaded.status, WaveStatus::Paused);

        let repo = Repo {
            path: "/tmp/repo".to_string(),
            repo_id: RepoId::parse("loopflowstudio/repo").expect("repo id"),
            name: "repo".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        store.upsert_repo(&repo).await.expect("upsert repo");
        assert_eq!(
            store
                .list_repos()
                .await
                .expect("list repos")
                .first()
                .map(|entry| entry.path.clone()),
            Some(repo.path.clone())
        );
        assert!(store
            .get_repo(&repo.path)
            .await
            .expect("get repo")
            .is_some());
        store
            .delete_repo(&repo.path)
            .await
            .expect("delete repo registration");
        assert!(store
            .get_repo(&repo.path)
            .await
            .expect("repo removed")
            .is_none());

        let parent = Repo {
            path: "/tmp/parent".to_string(),
            repo_id: RepoId::parse("loopflowstudio/parent").expect("parent repo id"),
            name: "parent".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        let child = Repo {
            path: "/tmp/child".to_string(),
            repo_id: RepoId::parse("loopflowstudio/child").expect("child repo id"),
            name: "child".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        store
            .upsert_repo(&parent)
            .await
            .expect("upsert parent repo");
        store.upsert_repo(&child).await.expect("upsert child repo");
        assert!(store
            .get_repo_by_repo_id(&parent.repo_id)
            .await
            .expect("get by repo id")
            .is_some());
        store
            .add_edge(&RepoEdge {
                parent_repo_id: parent.repo_id.clone(),
                child_repo_id: child.repo_id.clone(),
            })
            .await
            .expect("add edge");
        assert_eq!(store.list_edges().await.expect("list edges").len(), 1);
        assert_eq!(
            store
                .children(&parent.repo_id)
                .await
                .expect("list children")
                .len(),
            1
        );
        assert_eq!(
            store
                .parents(&child.repo_id)
                .await
                .expect("list parents")
                .len(),
            1
        );
        store
            .delete_repo(&parent.path)
            .await
            .expect("delete parent repo");
        assert!(store
            .list_edges()
            .await
            .expect("list edges after delete")
            .is_empty());

        let run = make_run(&wave, RunStatus::Running);
        store.create_run(&run).await.expect("create wave run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("get active")
            .is_some());

        let fork_run = ForkRun {
            id: LfdId::new(),
            run_id: run.id.clone(),
            step_index: 0,
            branch_index: 0,
            status: ForkRunStatus::Pending,
            worktree: "/tmp/branch".to_string(),
        };
        store
            .upsert_fork_run(&fork_run)
            .await
            .expect("upsert fork run");
        assert_eq!(
            store
                .list_fork_runs(&run.id, 0)
                .await
                .expect("list fork runs")
                .len(),
            1
        );
        let mut failed_run = run.clone();
        failed_run.status = RunStatus::Failed;
        store
            .update_run(&failed_run)
            .await
            .expect("update run to failed");
        let orphaned = store
            .list_orphaned_fork_runs()
            .await
            .expect("list orphaned fork runs");
        assert_eq!(orphaned.len(), 1);
        store.update_run(&run).await.expect("restore run status");
        let deleted_forks = store
            .delete_fork_runs(&run.id, 0)
            .await
            .expect("delete fork runs");
        assert_eq!(deleted_forks, 1);

        let summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            content: "summary".to_string(),
            source_hash: "abc".to_string(),
            token_budget: 100,
            agent: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_summary(&summary)
            .await
            .expect("upsert summary");
        assert!(store
            .get_summary(wave.id())
            .await
            .expect("get summary")
            .is_some());

        // Upsert replaces on same wave_id
        let updated_summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            content: "# Updated summary".to_string(),
            source_hash: "def456".to_string(),
            token_budget: 10000,
            agent: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_summary(&updated_summary)
            .await
            .expect("upsert updated summary");
        let reloaded = store
            .get_summary(wave.id())
            .await
            .expect("get updated summary")
            .expect("summary should exist");
        assert_eq!(reloaded.content, "# Updated summary");
        assert_eq!(reloaded.source_hash, "def456");

        // Chat memory block CRUD
        let block_a = ChatMemoryBlock {
            wave_id: wave.id().clone(),
            name: "preferences".to_string(),
            content: "Keep responses concise.".to_string(),
            position: 1,
            updated_at: Some(OffsetDateTime::now_utc()),
        };
        let block_b = ChatMemoryBlock {
            wave_id: wave.id().clone(),
            name: "project-context".to_string(),
            content: "Repo uses Rust + Swift.".to_string(),
            position: 0,
            updated_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_chat_memory_block(&block_a)
            .await
            .expect("upsert block a");
        store
            .upsert_chat_memory_block(&block_b)
            .await
            .expect("upsert block b");
        let blocks = store
            .list_chat_memory_blocks(wave.id())
            .await
            .expect("list blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "project-context");
        assert_eq!(blocks[1].name, "preferences");

        let block_a_updated = ChatMemoryBlock {
            content: "Prefer bullet points.".to_string(),
            position: 2,
            ..block_a
        };
        store
            .upsert_chat_memory_block(&block_a_updated)
            .await
            .expect("upsert updated block");
        let blocks = store
            .list_chat_memory_blocks(wave.id())
            .await
            .expect("list blocks after update");
        let updated = blocks
            .iter()
            .find(|block| block.name == "preferences")
            .expect("updated memory block should exist");
        assert_eq!(updated.content, "Prefer bullet points.");
        assert_eq!(updated.position, 2);

        store
            .delete_chat_memory_block(wave.id(), "project-context")
            .await
            .expect("delete block");
        let blocks = store
            .list_chat_memory_blocks(wave.id())
            .await
            .expect("list blocks after delete");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "preferences");

        store.delete_wave(wave.id()).await.expect("delete wave");
        assert!(store
            .get_wave(wave.id())
            .await
            .expect("get deleted wave")
            .is_none());
    }

    #[tokio::test]
    async fn sqlite_store_basic_suite() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");
        run_store_basic_suite(&store).await;
    }

    // A chord is a wave whose children point back at it via `parent_wave_id`.
    // `list_child_waves` returns those children (ordered, repos stitched); a
    // leaf wave returns none. This is the ancestry the WaveAgentTree needs.
    #[tokio::test]
    async fn sqlite_wave_ancestry_and_children() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let parent = make_wave("/chord");
        store.create_wave(&parent).await.expect("create parent");

        let child_a = make_wave("/repo-a").with_parent(parent.id().clone());
        let child_b = make_wave("/repo-b").with_parent(parent.id().clone());
        store.create_wave(&child_a).await.expect("create child a");
        store.create_wave(&child_b).await.expect("create child b");

        // Wave exposes its parent relation after a round-trip.
        let reloaded = store
            .get_wave(child_a.id())
            .await
            .expect("get child")
            .expect("child exists");
        assert_eq!(reloaded.parent_wave_id(), Some(parent.id()));

        // Chord contents = children where parent_wave_id = id, one per repo.
        let children = store
            .list_child_waves(parent.id())
            .await
            .expect("list children");
        assert_eq!(children.len(), 2);
        let repos: Vec<&str> = children.iter().map(|w| w.repo()).collect();
        assert!(repos.contains(&"/repo-a"));
        assert!(repos.contains(&"/repo-b"));

        // A leaf wave has no children.
        assert!(store
            .list_child_waves(child_a.id())
            .await
            .expect("leaf children")
            .is_empty());

        // The root wave itself has no parent.
        let root = store
            .get_wave(parent.id())
            .await
            .expect("get parent")
            .expect("parent exists");
        assert_eq!(root.parent_wave_id(), None);
    }

    /// A run_events row with no usage attached.
    fn event_row(run_id: &str, seq: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq,
            ts: seq,
            repo: Some("/repo".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: None,
            flow: None,
            skill: None,
            step_index: None,
            error: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
            duration_secs: None,
            provider: None,
            model: None,
        }
    }

    #[tokio::test]
    async fn pm_snapshot_replacement_is_atomic_per_wave() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("store should open");
        let mut snapshot = PmSnapshotRow {
            repo: "/repo".to_string(),
            wave: "product".to_string(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: 1,
            payload: "{\"version\":1}".to_string(),
        };
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("write snapshot");
        snapshot.synced_at = 2;
        snapshot.payload = "{\"version\":2}".to_string();
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("replace snapshot");

        assert_eq!(
            store
                .pm_snapshot("/repo".to_string(), "product".to_string())
                .await
                .expect("read snapshot"),
            Some(snapshot)
        );
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn a_closed_vocabulary_rejects_an_unknown_node() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");
        let row = event_row("bad-node", 0, "task", "started");
        let error = store
            .insert_run_event(&row)
            .expect_err("unknown node must violate the ledger contract");
        assert!(error.to_string().contains("CHECK constraint failed"));
    }

    #[test]
    fn store_open_rejects_a_recorded_migration_with_a_drifted_ledger() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite db");
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                     version TEXT PRIMARY KEY,
                     applied_at INTEGER NOT NULL
                 );
                 CREATE TABLE run_events (run_id TEXT NOT NULL);",
            )
            .unwrap();
            for migration in super::migrations::migrations() {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![migration.version],
                )
                .unwrap();
            }
        }

        let error = SqliteStore::new(&db_path)
            .expect_err("a ledger that lies about migration 057 must fail open");
        assert!(error.to_string().contains("process_id"));
    }

    #[tokio::test]
    async fn sqlite_health_check_succeeds() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        store.health_check().await.expect("sqlite health check");
    }

    // When lfd restarts, `fail_orphaned_runs` marks in-flight runs as Failed.
    // Without also resetting the wave's own status, the wave stays visually
    // "running" — the Loopflow sidebar and buttons stay disabled forever even
    // though no executor is attached. Cover the reset here.
    #[tokio::test]
    async fn fail_orphaned_runs_resets_stuck_wave_status() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Wave that was actively running when lfd died.
        let mut running_wave = make_wave("/repo-stuck-running");
        running_wave.set_status(WaveStatus::Running);
        store.create_wave(&running_wave).await.expect("create wave");
        let mut running_run = make_run(&running_wave, RunStatus::Running);
        store.create_run(&running_run).await.expect("create run");

        // Wave that was at an interactive waitpoint.
        let mut waiting_wave = make_wave("/repo-stuck-waiting");
        waiting_wave.set_status(WaveStatus::Waiting);
        store
            .create_wave(&waiting_wave)
            .await
            .expect("create waiting wave");
        let mut waiting_run = make_run(&waiting_wave, RunStatus::Waiting);
        store
            .create_run(&waiting_run)
            .await
            .expect("create waiting run");

        // Paused wave with no active run — must be left alone.
        let mut paused_wave = make_wave("/repo-paused");
        paused_wave.set_status(WaveStatus::Paused);
        store
            .create_wave(&paused_wave)
            .await
            .expect("create paused wave");

        let _ = store.fail_orphaned_runs().await.expect("fail orphans");

        running_run = store
            .get_run(&running_run.id)
            .await
            .expect("get run")
            .expect("run exists");
        assert_eq!(running_run.status, RunStatus::Failed);
        waiting_run = store
            .get_run(&waiting_run.id)
            .await
            .expect("get waiting run")
            .expect("run exists");
        assert_eq!(waiting_run.status, RunStatus::Failed);

        let running_after = store
            .get_wave(running_wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(
            running_after.status(),
            WaveStatus::Idle,
            "wave whose run was orphaned should be reset to Idle"
        );
        let waiting_after = store
            .get_wave(waiting_wave.id())
            .await
            .expect("get waiting wave")
            .expect("wave exists");
        assert_eq!(
            waiting_after.status(),
            WaveStatus::Idle,
            "waiting-state wave should also be reset to Idle"
        );
        let paused_after = store
            .get_wave(paused_wave.id())
            .await
            .expect("get paused wave")
            .expect("wave exists");
        assert_eq!(
            paused_after.status(),
            WaveStatus::Paused,
            "paused wave must keep its status across orphan cleanup"
        );
    }

    #[tokio::test]
    async fn provider_token_round_trip() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Initially empty
        assert!(store
            .list_provider_tokens()
            .await
            .expect("list empty")
            .is_empty());
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get missing")
            .is_none());

        // Upsert a token
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_abc123".to_string(),
            refresh_token: Some("ghr_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        // Read it back
        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token should exist");
        assert_eq!(loaded.provider, "github");
        assert_eq!(loaded.access_token, "gho_abc123");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_refresh"));
        assert_eq!(loaded.oauth_client_id.as_deref(), Some("github-client"));
        assert_eq!(loaded.expires_at, Some(1700000000));
        assert_eq!(loaded.login.as_deref(), Some("octocat"));

        // Upsert overwrites
        let updated = super::ProviderToken {
            access_token: "gho_new456".to_string(),
            refresh_token: None,
            updated_at: 1699500000,
            ..token.clone()
        };
        store
            .upsert_provider_token(&updated)
            .await
            .expect("upsert update");
        let reloaded = store
            .get_provider_token("github")
            .await
            .expect("get updated")
            .expect("exists");
        assert_eq!(reloaded.access_token, "gho_new456");
        assert!(reloaded.refresh_token.is_none());

        assert_eq!(reloaded.credential_type, super::CredentialType::OAuth);

        // Add a second provider and list
        let claude_token = super::ProviderToken {
            provider: "claude".to_string(),
            access_token: "sk-ant-key".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&claude_token)
            .await
            .expect("upsert claude");
        let all = store.list_provider_tokens().await.expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].provider, "claude");
        assert_eq!(all[1].provider, "github");

        // Delete one
        store
            .delete_provider_token("github")
            .await
            .expect("delete github");
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get deleted")
            .is_none());
        assert_eq!(
            store
                .list_provider_tokens()
                .await
                .expect("list after delete")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn provider_tokens_are_encrypted_at_rest_in_sqlite() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path.clone());
        let store = super::open_store(&config).await.expect("store should open");
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_secret_access".to_string(),
            refresh_token: Some("ghr_secret_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, raw_refresh, encrypted): (String, Option<String>, bool) = conn
            .query_row(
                "SELECT access_token, refresh_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query provider token");

        assert_ne!(raw_access, "gho_secret_access");
        assert_ne!(
            raw_refresh.as_deref(),
            Some("ghr_secret_refresh"),
            "refresh token should be encrypted"
        );
        assert!(encrypted, "encrypted flag should be true");
    }

    #[tokio::test]
    async fn sqlite_open_migrates_existing_plaintext_provider_tokens() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite db");
            super::migrations::apply_sqlite(&conn).expect("apply migrations");
            conn.execute(
                "INSERT INTO provider_tokens
                 (provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![
                    "github",
                    "gho_plaintext",
                    "ghr_plaintext",
                    1700000000_i64,
                    "octocat",
                    1699000000_i64,
                    "oauth",
                ],
            )
            .expect("insert plaintext token");
        }

        let store = super::open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("open store");

        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token exists");
        assert_eq!(loaded.access_token, "gho_plaintext");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_plaintext"));

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, encrypted): (String, bool) = conn
            .query_row(
                "SELECT access_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query provider token");
        assert_ne!(raw_access, "gho_plaintext");
        assert!(encrypted);
    }

    #[tokio::test]
    async fn active_run_excludes_failed_runs() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-active");
        store.create_wave(&wave).await.expect("create wave");

        let mut run = make_run(&wave, RunStatus::Running);
        store.create_run(&run).await.expect("create running run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active run")
            .is_some());

        run.status = RunStatus::Failed;
        run.error = Some("failed".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        store.update_run(&run).await.expect("update run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active after fail")
            .is_none());
        assert_eq!(
            store
                .get_latest_run(wave.id())
                .await
                .expect("latest run")
                .expect("run exists")
                .status,
            RunStatus::Failed
        );

        let mut ci_fix = make_run(&wave, RunStatus::Running);
        ci_fix.flow = "ci-fix".to_string();
        store.create_run(&ci_fix).await.expect("create ci-fix run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active with ci-fix")
            .is_some());

        store.delete_wave(wave.id()).await.expect("delete wave");
    }
}
