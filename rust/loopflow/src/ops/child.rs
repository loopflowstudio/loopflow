//! Shared durable control submission for Project and Task Sessions.
//!
//! The public nouns stay explicit. This module owns only the protocol that is
//! already common at both edges: persist one command, supersede or version its
//! directive atomically, wake the right child, and report the durable receipt.

use std::time::Duration;

use crate::lfdb::SharedStore;
use crate::project_session::{ProjectEventKind, ProjectSession, ProjectSessionStatus};
use crate::task::{
    ChildCommand, ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandSource,
    ChildCommandState, ChildDirective, ChildRef, TaskEventKind, TaskSession, TaskSessionStatus,
};

use super::{OpsError, OpsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ChildReceiptUntil {
    Applied,
    Incorporated,
}

#[derive(Debug)]
pub(crate) enum ChildSession {
    Project(Box<ProjectSession>),
    Task(Box<TaskSession>),
}

impl ChildSession {
    fn target(&self) -> ChildRef {
        match self {
            Self::Project(session) => ChildRef::Project(session.id.clone()),
            Self::Task(session) => ChildRef::Task(session.id.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Project(session) => format!("Project {}", session.project.slug),
            Self::Task(session) => format!("Task {}", session.issue.identifier),
        }
    }

    fn status(&self) -> &'static str {
        match self {
            Self::Project(session) => session.status.as_str(),
            Self::Task(session) => session.status.as_str(),
        }
    }

    fn is_terminal(&self) -> bool {
        match self {
            Self::Project(session) => session.status.is_terminal(),
            Self::Task(session) => session.status.is_terminal(),
        }
    }

    fn is_process_active(&self) -> bool {
        match self {
            Self::Project(session) => session.status.is_process_active(),
            Self::Task(session) => session.status.is_process_active(),
        }
    }

    fn current_directive_version(&self) -> u32 {
        match self {
            Self::Project(session) => session.current_directive_version,
            Self::Task(session) => session.current_directive_version,
        }
    }

    fn set_current_directive_version(&mut self, version: u32) {
        match self {
            Self::Project(session) => session.current_directive_version = version,
            Self::Task(session) => session.current_directive_version = version,
        }
    }

    async fn refresh(&mut self, store: &SharedStore) -> OpsResult<()> {
        *self = match self {
            Self::Project(session) => Self::Project(Box::new(
                store
                    .get_project_session(&session.id)
                    .await
                    .map_err(child_error)?
                    .ok_or_else(|| child_error("Project Session disappeared"))?,
            )),
            Self::Task(session) => Self::Task(Box::new(
                store
                    .get_task_session(&session.id)
                    .await
                    .map_err(child_error)?
                    .ok_or_else(|| child_error("Task Session disappeared"))?,
            )),
        };
        Ok(())
    }

    async fn launch(&mut self, store: &SharedStore) -> OpsResult<()> {
        match self {
            Self::Project(session) => super::project::launch_project_process(store, session).await,
            Self::Task(session) => super::task::relaunch_inactive_process(store, session).await,
        }
    }

    async fn append_command_event(
        &self,
        store: &SharedStore,
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
    ) -> OpsResult<()> {
        match self {
            Self::Project(session) => store
                .append_project_event(
                    &session.id,
                    &ProjectEventKind::CommandChanged {
                        command_id,
                        state,
                        effect,
                        error: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
            Self::Task(session) => store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::CommandChanged {
                        command_id,
                        state,
                        effect,
                        error: None,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
        }
    }

    async fn append_directive_event(
        &self,
        store: &SharedStore,
        directive: &ChildDirective,
    ) -> OpsResult<()> {
        match self {
            Self::Project(session) => store
                .append_project_event(
                    &session.id,
                    &ProjectEventKind::DirectiveChanged {
                        directive_id: directive.id.clone(),
                        version: directive.version,
                        directive_kind: directive.kind,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
            Self::Task(session) => store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::DirectiveChanged {
                        directive_id: directive.id.clone(),
                        version: directive.version,
                        directive_kind: directive.kind,
                    },
                )
                .await
                .map(|_| ())
                .map_err(child_error),
        }
    }

    async fn abandon(&mut self, store: &SharedStore, reason: &str) -> OpsResult<()> {
        match self {
            Self::Project(session) => {
                let from = session.status;
                session.set_status(
                    ProjectSessionStatus::Abandoned,
                    format!("Project Session explicitly abandoned: {reason}"),
                );
                store
                    .update_project_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_project_event(
                        &session.id,
                        &ProjectEventKind::StatusChanged {
                            from,
                            to: ProjectSessionStatus::Abandoned,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
            Self::Task(session) => {
                let from = session.status;
                session.set_status(
                    TaskSessionStatus::Abandoned,
                    format!("Task Session explicitly abandoned: {reason}"),
                );
                store
                    .update_task_session(session)
                    .await
                    .map_err(child_error)?;
                store
                    .append_task_event(
                        &session.id,
                        &TaskEventKind::StatusChanged {
                            from,
                            to: TaskSessionStatus::Abandoned,
                            reason: session.status_reason.clone(),
                        },
                    )
                    .await
                    .map_err(child_error)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildControlResult {
    pub session_id: String,
    pub command_id: String,
    pub directive_version: Option<u32>,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub incorporated: bool,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub incorporated_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

pub(crate) async fn queue_command(
    store: &SharedStore,
    mut session: ChildSession,
    source: ChildCommandSource,
    kind: ChildCommandKind,
) -> OpsResult<ChildControlResult> {
    if session.is_terminal() {
        return Err(child_error(format!(
            "{} is {}; terminal Sessions cannot accept commands",
            session.label(),
            session.status()
        )));
    }

    let command = ChildCommand::new(session.target(), source, kind);
    let wait_for_resolution = !matches!(&command.kind, ChildCommandKind::FollowUp { .. });
    let replacement = match &command.kind {
        ChildCommandKind::Steer { text } => Some(text.clone()),
        ChildCommandKind::Interrupt {
            replacement: Some(text),
        } => Some(text.clone()),
        _ => None,
    };

    let (command, created, superseded, directive) = if let Some(text) = replacement {
        let directive = ChildDirective::replacement(
            session.target(),
            session.current_directive_version() + 1,
            text,
            command.source.clone(),
            command.id.clone(),
        );
        let superseded = store
            .create_child_command_with_directive(&command, &directive)
            .await
            .map_err(child_error)?;
        session.set_current_directive_version(directive.version);
        (command, true, superseded, Some(directive))
    } else if matches!(&command.kind, ChildCommandKind::Decide { .. }) {
        let (command, created) = store
            .ensure_child_decision_command(&command)
            .await
            .map_err(child_error)?;
        (command, created, Vec::new(), None)
    } else if matches!(&command.kind, ChildCommandKind::Interrupt { .. }) {
        let superseded = store
            .supersede_and_create_child_command(&command)
            .await
            .map_err(child_error)?;
        (command, true, superseded, None)
    } else {
        store
            .create_child_command(&command)
            .await
            .map_err(child_error)?;
        (command, true, Vec::new(), None)
    };

    if !created {
        if !command.state.is_terminal() && !session.is_process_active() {
            session.launch(store).await?;
        }
        let receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
        return control_result(store, &command, receipt).await;
    }

    for command_id in superseded {
        session
            .append_command_event(store, command_id, ChildCommandState::Superseded, None)
            .await?;
    }
    if let Some(directive) = &directive {
        session.append_directive_event(store, directive).await?;
    }
    session
        .append_command_event(
            store,
            command.id.clone(),
            ChildCommandState::Persisted,
            command.effect,
        )
        .await?;

    if let ChildCommandKind::Abandon { reason } = &command.kind {
        if !session.is_process_active() {
            store
                .accept_child_command(&command.id, None)
                .await
                .map_err(child_error)?;
            session
                .append_command_event(store, command.id.clone(), ChildCommandState::Accepted, None)
                .await?;
            session.abandon(store, reason).await?;
            let receipt = read_receipt(store, &command.id).await?;
            return control_result(store, &command, receipt).await;
        }
    }

    if !session.is_process_active() {
        session.launch(store).await?;
    }
    let mut receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
    if matches!(
        receipt.state,
        ChildCommandState::Persisted | ChildCommandState::Claimed
    ) {
        session.refresh(store).await?;
        if !session.is_process_active() && !session.is_terminal() {
            session.launch(store).await?;
            receipt = resolve_receipt(store, &command.id, wait_for_resolution).await?;
        }
    }
    control_result(store, &command, receipt).await
}

pub(crate) async fn control_result(
    store: &SharedStore,
    command: &ChildCommand,
    receipt: ChildCommand,
) -> OpsResult<ChildControlResult> {
    let directive = store
        .child_directive_for_command(&command.id)
        .await
        .map_err(child_error)?;
    Ok(ChildControlResult {
        session_id: command.target.target_id().to_string(),
        command_id: command.id.to_string(),
        directive_version: directive.as_ref().map(|directive| directive.version),
        state: receipt.state,
        effect: receipt.effect,
        incorporated: directive
            .as_ref()
            .is_some_and(|directive| directive.incorporated_at.is_some()),
        generation: receipt.claimed_by_generation,
        accepted_at: receipt.accepted_at,
        incorporated_at: directive.and_then(|directive| directive.incorporated_at),
        error: receipt.error,
    })
}

async fn resolve_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
    wait: bool,
) -> OpsResult<ChildCommand> {
    if wait {
        Ok(wait_for_receipt(store, command_id, Duration::from_secs(2))
            .await?
            .0)
    } else {
        read_receipt(store, command_id).await
    }
}

pub(crate) async fn read_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
) -> OpsResult<ChildCommand> {
    store
        .get_child_command(command_id)
        .await
        .map_err(child_error)?
        .ok_or_else(|| child_error(format!("child command {command_id} disappeared")))
}

pub(crate) async fn wait_for_receipt(
    store: &SharedStore,
    command_id: &ChildCommandId,
    timeout: Duration,
) -> OpsResult<(ChildCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_receipt(store, command_id).await?;
        if command.state.is_terminal() {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub(crate) async fn wait_for_receipt_condition(
    store: &SharedStore,
    command_id: &ChildCommandId,
    until: ChildReceiptUntil,
    timeout: Duration,
) -> OpsResult<(ChildCommand, bool)> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let command = read_receipt(store, command_id).await?;
        if matches!(
            command.state,
            ChildCommandState::Failed | ChildCommandState::Superseded
        ) {
            return Ok((command, false));
        }
        let settled = match until {
            ChildReceiptUntil::Applied => command.state.is_terminal(),
            ChildReceiptUntil::Incorporated => store
                .child_directive_for_command(command_id)
                .await
                .map_err(child_error)?
                .ok_or_else(|| {
                    child_error(format!(
                        "child command {command_id} does not carry a directive to incorporate"
                    ))
                })?
                .incorporated_at
                .is_some(),
        };
        if settled {
            return Ok((command, false));
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok((command, true));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn child_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Wave, WaveStatus};
    use crate::lfdb::{open_store, StorageConfig};
    use crate::project_session::{
        ProjectProcess, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::task::{
        ChildCommandKind, ChildCommandSource, ChildCommandState, LinearProjectId, LinearProjectRef,
    };

    use super::{queue_command, ChildSession};

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            goal: "keep child control coherent".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_project(wave: &Wave, status: ProjectSessionStatus) -> ProjectSession {
        let now = OffsetDateTime::now_utc();
        let active = status.is_process_active();
        ProjectSession {
            id: ProjectSessionId::new(),
            project: LinearProjectRef {
                id: LinearProjectId::new(format!("project-{}", LfdId::new())).unwrap(),
                slug: format!("project-{}", LfdId::new()),
                name: "Child control".to_string(),
                context: "Keep one control protocol.".to_string(),
            },
            wave_id: wave.id().clone(),
            wave: wave.name().to_string(),
            repo: wave.repo().to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status,
            status_reason: "test project session".to_string(),
            status_at: now,
            iteration: 1,
            task_event_cursor: 0,
            state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: active.then(|| "thread-project".to_string()),
            process: active.then_some(ProjectProcess {
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
    async fn project_follow_up_returns_once_durable() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Running);
        store.create_project_session(&project).await.unwrap();

        let result = queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "Inspect the boundary next".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(result.state, ChildCommandState::Persisted);
        assert_eq!(result.accepted_at, None);
    }

    #[tokio::test]
    async fn inactive_project_abandonment_does_not_launch_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = make_wave(dir.path().to_str().unwrap());
        store.create_wave(&wave).await.unwrap();
        let project = make_project(&wave, ProjectSessionStatus::Created);
        let project_id = project.id.clone();
        store.create_project_session(&project).await.unwrap();

        let result = queue_command(
            &store,
            ChildSession::Project(Box::new(project)),
            ChildCommandSource::Human,
            ChildCommandKind::Abandon {
                reason: "The measured bet no longer matters".to_string(),
            },
        )
        .await
        .unwrap();
        let persisted = store
            .get_project_session(&project_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result.state, ChildCommandState::Accepted);
        assert_eq!(persisted.status, ProjectSessionStatus::Abandoned);
        assert_eq!(persisted.process, None);
    }
}
