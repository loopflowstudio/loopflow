use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::project_session::{
    ChildEventPayload, ProjectCommand, ProjectCommandId, ProjectCommandKind, ProjectDecisionId,
    ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus, SessionSupervisor,
};
use crate::task::{TaskCommandEffect, TaskCommandState, TaskSessionStatus};

#[derive(Debug)]
struct PendingInput {
    command_id: Option<ProjectCommandId>,
    text: String,
    effect: TaskCommandEffect,
    decision: Option<DecisionResolution>,
}

#[derive(Debug)]
struct DecisionResolution {
    decision_id: ProjectDecisionId,
    choice: String,
    message: Option<String>,
}

enum CommandStop {
    Interrupted,
    Abandoned(String),
}

pub async fn run_project_session(session_id: ProjectSessionId, generation: u32) -> Result<()> {
    let result = run_project_session_inner(session_id.clone(), generation).await;
    if let Err(error) = &result {
        record_unhandled_failure(&session_id, generation, error).await;
    }
    result
}

async fn run_project_session_inner(session_id: ProjectSessionId, generation: u32) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let mut session = store
        .get_project_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Project Session {session_id} not found"))?;
    if session.process.as_ref().map(|process| process.generation) != Some(generation) {
        anyhow::bail!("Project Session {session_id} generation {generation} is not current");
    }
    if let Some(process) = &mut session.process {
        process.pid = Some(std::process::id());
    }
    set_and_record_status(
        &store,
        &mut session,
        ProjectSessionStatus::Running,
        "project pursuit turn is active",
    )
    .await?;
    store
        .append_project_event(&session.id, &ProjectEventKind::Started)
        .await?;

    let observations = consume_task_observations(&store, &mut session).await?;
    let seed = project_seed(&session, &observations);
    let mut prepared = crate::lf::commands::run::prepare_harness_turn(
        "project_pursue",
        &seed,
        &session.wave,
        None,
    )?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    prepared.config.agent = Some(session.agent.clone());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    harness.start(&prepared.config).await?;
    session.provider = harness_name;
    session.provider_session_id = harness.provider_session_id();
    store.update_project_session(&session).await?;

    let mut pending = VecDeque::new();
    let commands = store
        .claim_project_commands(&session.id, generation)
        .await?;
    if let Some(stop) = absorb_commands(
        &store,
        &session,
        commands,
        harness.as_mut(),
        false,
        &mut pending,
    )
    .await?
    {
        return finish_command_stop(&store, &mut session, harness.as_mut(), stop).await;
    }
    let input = if generation > 1 {
        pending.pop_front().unwrap_or(PendingInput {
            command_id: None,
            text: prepared.input,
            effect: TaskCommandEffect::NextTurn,
            decision: None,
        })
    } else {
        PendingInput {
            command_id: None,
            text: prepared.input,
            effect: TaskCommandEffect::NextTurn,
            decision: None,
        }
    };
    apply_input(&store, &session, harness.as_mut(), input).await?;

    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    loop {
        tokio::select! {
            _ = poll.tick() => {
                let commands = store.claim_project_commands(&session.id, generation).await?;
                if let Some(stop) = absorb_commands(
                    &store,
                    &session,
                    commands,
                    harness.as_mut(),
                    true,
                    &mut pending,
                ).await? {
                    return finish_command_stop(&store, &mut session, harness.as_mut(), stop).await;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(&store, &mut session, harness.as_mut(), "provider event stream closed").await;
                };
                if session.provider_session_id.is_none() {
                    session.provider_session_id = harness.provider_session_id();
                    store.update_project_session(&session).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            return finish_failed(&store, &mut session, harness.as_mut(), "provider turn failed").await;
                        }
                        if let Some(input) = pending.pop_front() {
                            apply_input(&store, &session, harness.as_mut(), input).await?;
                            continue;
                        }
                        let observations = consume_task_observations(&store, &mut session).await?;
                        if !observations.is_empty() {
                            apply_input(
                                &store,
                                &session,
                                harness.as_mut(),
                                PendingInput {
                                    command_id: None,
                                    text: format!(
                                        "New supervised Task observations arrived. Continue the same Project iteration:\n{}",
                                        observations.join("\n")
                                    ),
                                    effect: TaskCommandEffect::NextTurn,
                                    decision: None,
                                },
                            ).await?;
                            continue;
                        }
                        session.iteration += 1;
                        let summary = bounded_summary(&last_text);
                        store.append_project_event(
                            &session.id,
                            &ProjectEventKind::IterationCompleted {
                                iteration: session.iteration,
                                summary: summary.clone(),
                            },
                        ).await?;
                        let outcome = inspect_outcome(&store, &session).await?;
                        if outcome.status == ProjectSessionStatus::Running {
                            session.state_fingerprint = Some(outcome.fingerprint);
                            session.updated_at = time::OffsetDateTime::now_utc();
                            store.update_project_session(&session).await?;
                            last_text.clear();
                            apply_input(
                                &store,
                                &session,
                                harness.as_mut(),
                                PendingInput {
                                    command_id: None,
                                    text: "Project state changed and open KRs remain. Run the next clarify/pursue/mutate iteration now.".to_string(),
                                    effect: TaskCommandEffect::NextTurn,
                                    decision: None,
                                },
                            ).await?;
                            continue;
                        }
                        session.state_fingerprint = Some(outcome.fingerprint);
                        store.update_project_session(&session).await?;
                        let (commands, stopped) = store.claim_project_commands_or_stop(
                            &session.id,
                            generation,
                            outcome.status,
                            outcome.reason.clone(),
                        ).await?;
                        if !commands.is_empty() {
                            if let Some(stop) = absorb_commands(
                                &store,
                                &session,
                                commands,
                                harness.as_mut(),
                                false,
                                &mut pending,
                            ).await? {
                                return finish_command_stop(&store, &mut session, harness.as_mut(), stop).await;
                            }
                            if let Some(input) = pending.pop_front() {
                                apply_input(&store, &session, harness.as_mut(), input).await?;
                            }
                            continue;
                        }
                        let from = session.status;
                        session = stopped.ok_or_else(|| anyhow!("project boundary stopped without state"))?;
                        let _ = harness.stop().await;
                        store.append_project_event(
                            &session.id,
                            &ProjectEventKind::StatusChanged {
                                from,
                                to: session.status,
                                reason: session.status_reason.clone(),
                            },
                        ).await?;
                        if session.status == ProjectSessionStatus::Completed {
                            store.append_project_event(
                                &session.id,
                                &ProjectEventKind::Completed { summary },
                            ).await?;
                        }
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message } => {
                        return finish_failed(
                            &store,
                            &mut session,
                            harness.as_mut(),
                            &format!("{code}: {message}"),
                        ).await;
                    }
                    ConversationEvent::TurnStarted { .. }
                    | ConversationEvent::ItemStarted { .. }
                    | ConversationEvent::ItemUpdated { .. }
                    | ConversationEvent::ItemCompleted { .. }
                    | ConversationEvent::ReasoningDelta { .. }
                    | ConversationEvent::DiffUpdated { .. }
                    | ConversationEvent::TurnUsage { .. }
                    | ConversationEvent::SuggestedActions { .. }
                    | ConversationEvent::StatusChanged { .. } => {}
                }
            }
        }
    }
}

struct ProjectOutcome {
    status: ProjectSessionStatus,
    reason: String,
    fingerprint: String,
}

async fn inspect_outcome(store: &SharedStore, session: &ProjectSession) -> Result<ProjectOutcome> {
    let repo = session.repo.clone();
    let project_id = session.project.id.as_str().to_string();
    let resolved = tokio::task::spawn_blocking(move || {
        crate::ops::task_pm::resolve_project(
            std::path::Path::new(&repo),
            &project_id,
            crate::ops::pm::PmRefresh::Never,
        )
    })
    .await
    .map_err(|error| anyhow!(error.to_string()))??;
    let tasks = store
        .list_task_sessions(Some(&session.wave_id))
        .await?
        .into_iter()
        .filter(|task| {
            task.project.id.as_str() == session.project.id.as_str()
                && matches!(
                    &task.supervisor,
                    SessionSupervisor::Project { session_id } if session_id == &session.id
                )
        })
        .collect::<Vec<_>>();
    let fingerprint_payload = serde_json::json!({
        "project": resolved.project,
        "tasks": tasks.iter().map(|task| (&task.id, task.status, &task.updated_at)).collect::<Vec<_>>(),
    });
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&fingerprint_payload)?));
    if resolved.project.krs.iter().all(|kr| kr.holds) {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Completed,
            reason: "every current Project KR holds".to_string(),
            fingerprint,
        });
    }
    if tasks.iter().any(|task| {
        matches!(
            task.status,
            TaskSessionStatus::Created
                | TaskSessionStatus::Starting
                | TaskSessionStatus::Running
                | TaskSessionStatus::Submitted
        )
    }) {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Waiting,
            reason: "supervised Tasks are active; waiting for typed Task observations".to_string(),
            fingerprint,
        });
    }
    if session.state_fingerprint.as_deref() == Some(&fingerprint) {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Blocked,
            reason: "open KRs remain but a complete iteration changed no PM or Task state"
                .to_string(),
            fingerprint,
        });
    }
    Ok(ProjectOutcome {
        status: ProjectSessionStatus::Running,
        reason: "Project state changed; another iteration is actionable".to_string(),
        fingerprint,
    })
}

async fn consume_task_observations(
    store: &SharedStore,
    session: &mut ProjectSession,
) -> Result<Vec<String>> {
    let supervisor = SessionSupervisor::Project {
        session_id: session.id.clone(),
    };
    let observations = store.pending_observations(&supervisor).await?;
    let mut prompts = Vec::new();
    for observation in observations {
        let event = match &observation.payload {
            ChildEventPayload::Task { event } => event,
            _ => continue,
        };
        let inserted = store
            .consume_task_observation_for_project(&session.id, &observation)
            .await?;
        if inserted {
            prompts.push(serde_json::to_string(event)?);
        }
        session.task_event_cursor = session.task_event_cursor.max(observation.id);
    }
    Ok(prompts)
}

async fn absorb_commands(
    store: &SharedStore,
    session: &ProjectSession,
    commands: Vec<ProjectCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>> {
    for command in commands {
        match command.kind {
            ProjectCommandKind::FollowUp { text } => pending.push_back(PendingInput {
                command_id: Some(command.id),
                text,
                effect: TaskCommandEffect::NextTurn,
                decision: None,
            }),
            ProjectCommandKind::Steer { text }
                if turn_active && harness.capabilities().supports_steer =>
            {
                apply_input(
                    store,
                    session,
                    harness,
                    PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::LiveSteer,
                        decision: None,
                    },
                )
                .await?;
            }
            ProjectCommandKind::Steer { text } => {
                if turn_active {
                    if let Err(error) = harness.interrupt().await {
                        fail_command(
                            store,
                            session,
                            &command.id,
                            Some(TaskCommandEffect::Replacement),
                            &error.to_string(),
                        )
                        .await?;
                        return Err(error);
                    }
                }
                pending.push_back(PendingInput {
                    command_id: Some(command.id),
                    text,
                    effect: TaskCommandEffect::Replacement,
                    decision: None,
                });
            }
            ProjectCommandKind::Interrupt { replacement } => {
                if turn_active {
                    if let Err(error) = harness.interrupt().await {
                        fail_command(
                            store,
                            session,
                            &command.id,
                            replacement.as_ref().map(|_| TaskCommandEffect::Replacement),
                            &error.to_string(),
                        )
                        .await?;
                        return Err(error);
                    }
                }
                if let Some(text) = replacement {
                    pending.clear();
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Replacement,
                        decision: None,
                    });
                } else {
                    accept_command(store, session, &command.id, None).await?;
                    return Ok(Some(CommandStop::Interrupted));
                }
            }
            ProjectCommandKind::Resume { message } => {
                if let Some(text) = message {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::NextTurn,
                        decision: None,
                    });
                } else {
                    accept_command(store, session, &command.id, None).await?;
                }
            }
            ProjectCommandKind::Decide {
                decision_id,
                choice,
                message,
            } => {
                let text = format!(
                    "Decision {decision_id}: {choice}{}",
                    message
                        .as_ref()
                        .map(|message| format!("\n{message}"))
                        .unwrap_or_default()
                );
                let resolution = DecisionResolution {
                    decision_id,
                    choice,
                    message,
                };
                if turn_active && harness.capabilities().supports_steer {
                    apply_input(
                        store,
                        session,
                        harness,
                        PendingInput {
                            command_id: Some(command.id),
                            text,
                            effect: TaskCommandEffect::Decision,
                            decision: Some(resolution),
                        },
                    )
                    .await?;
                } else {
                    if turn_active {
                        if let Err(error) = harness.interrupt().await {
                            fail_command(
                                store,
                                session,
                                &command.id,
                                Some(TaskCommandEffect::Decision),
                                &error.to_string(),
                            )
                            .await?;
                            return Err(error);
                        }
                    }
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Decision,
                        decision: Some(resolution),
                    });
                }
            }
            ProjectCommandKind::Abandon { reason } => {
                accept_command(store, session, &command.id, None).await?;
                return Ok(Some(CommandStop::Abandoned(reason)));
            }
        }
    }
    Ok(None)
}

async fn apply_input(
    store: &SharedStore,
    session: &ProjectSession,
    harness: &mut dyn Harness,
    input: PendingInput,
) -> Result<()> {
    if let Err(error) = harness.send_input(&input.text).await {
        if let Some(command_id) = input.command_id {
            fail_command(
                store,
                session,
                &command_id,
                Some(input.effect),
                &error.to_string(),
            )
            .await?;
        }
        return Err(error);
    }
    if let Some(command_id) = input.command_id {
        accept_command(store, session, &command_id, Some(input.effect)).await?;
    }
    if let Some(decision) = input.decision {
        store
            .append_project_event(
                &session.id,
                &ProjectEventKind::DecisionResolved {
                    decision_id: decision.decision_id,
                    choice: decision.choice,
                    message: decision.message,
                },
            )
            .await?;
    }
    Ok(())
}

async fn fail_command(
    store: &SharedStore,
    session: &ProjectSession,
    command_id: &ProjectCommandId,
    effect: Option<TaskCommandEffect>,
    error: &str,
) -> Result<()> {
    store
        .fail_project_command(command_id, effect, error.to_string())
        .await?;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::CommandChanged {
                command_id: command_id.clone(),
                state: TaskCommandState::Failed,
                effect,
                error: Some(error.to_string()),
            },
        )
        .await?;
    Ok(())
}

async fn accept_command(
    store: &SharedStore,
    session: &ProjectSession,
    command_id: &ProjectCommandId,
    effect: Option<TaskCommandEffect>,
) -> Result<()> {
    store.accept_project_command(command_id, effect).await?;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::CommandChanged {
                command_id: command_id.clone(),
                state: TaskCommandState::Accepted,
                effect,
                error: None,
            },
        )
        .await?;
    Ok(())
}

async fn set_and_record_status(
    store: &SharedStore,
    session: &mut ProjectSession,
    status: ProjectSessionStatus,
    reason: impl Into<String>,
) -> Result<()> {
    let from = session.status;
    session.set_status(status, reason);
    store.update_project_session(session).await?;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::StatusChanged {
                from,
                to: status,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    Ok(())
}

async fn finish_failed(
    store: &SharedStore,
    session: &mut ProjectSession,
    harness: &mut dyn Harness,
    error: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    set_and_record_status(store, session, ProjectSessionStatus::Failed, error).await?;
    store
        .append_project_event(
            &session.id,
            &ProjectEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
    anyhow::bail!(error.to_string())
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut ProjectSession,
    harness: &mut dyn Harness,
    reason: String,
) -> Result<()> {
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    set_and_record_status(
        store,
        session,
        ProjectSessionStatus::Abandoned,
        format!("Project Session explicitly abandoned: {reason}"),
    )
    .await
}

async fn finish_command_stop(
    store: &SharedStore,
    session: &mut ProjectSession,
    harness: &mut dyn Harness,
    stop: CommandStop,
) -> Result<()> {
    match stop {
        CommandStop::Interrupted => {
            let _ = harness.stop().await;
            set_and_record_status(
                store,
                session,
                ProjectSessionStatus::Waiting,
                "Project turn interrupted; waiting for resume or another instruction",
            )
            .await
        }
        CommandStop::Abandoned(reason) => finish_abandoned(store, session, harness, reason).await,
    }
}

async fn record_unhandled_failure(
    session_id: &ProjectSessionId,
    generation: u32,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_project_session(session_id).await else {
        return;
    };
    if session.process.as_ref().map(|process| process.generation) != Some(generation)
        || !session.status.is_process_active()
    {
        return;
    }
    let _ = set_and_record_status(
        &store,
        &mut session,
        ProjectSessionStatus::Failed,
        format!("project runner failed: {error}"),
    )
    .await;
}

fn project_seed(session: &ProjectSession, observations: &[String]) -> String {
    let observations = if observations.is_empty() {
        "none".to_string()
    } else {
        observations.join("\n")
    };
    format!(
        "Pursue Linear Project {name} ({project_id}) in wave/{wave}.\n\n{context}\n\nProject Session: {session_id}\nIteration: {iteration}\nPM snapshot synced at: {synced_at}\nSupervised Task observations:\n{observations}\n\nThis is one clarify/pursue/mutate iteration. Read and update only this Linear Project through `lf pm`. Create or select concrete Linear tasks, run file-writing work with `lf task run <issue-id>`, and supervise those Task Sessions. Do not edit repository files from the Wave home. Return a concise evidence summary; the runner decides complete, wait, repeat, or block from authoritative PM and Task state.",
        name = session.project.name,
        project_id = session.project.id.as_str(),
        wave = session.wave,
        context = session.project.context,
        session_id = session.id,
        iteration = session.iteration + 1,
        synced_at = session.pm_snapshot_synced_at,
    )
}

fn bounded_summary(text: &str) -> String {
    const MAX_CHARS: usize = 2_000;
    let text = text.trim();
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }
    let mut summary: String = text.chars().take(MAX_CHARS - 1).collect();
    summary.push('…');
    summary
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use time::OffsetDateTime;

    use super::{absorb_commands, apply_input, CommandStop};
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Capabilities, Harness};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::Wave;
    use crate::lfdb::{open_store, SharedStore, StorageConfig};
    use crate::project_session::{
        ProjectCommand, ProjectCommandKind, ProjectCommandSource, ProjectDecisionId,
        ProjectProcess, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::task::{LinearProjectId, LinearProjectRef, TaskCommandEffect, TaskCommandState};

    struct ScriptedHarness {
        supports_steer: bool,
        sent: Vec<String>,
        interrupts: usize,
    }

    #[async_trait]
    impl Harness for ScriptedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            self.sent.push(content.to_string());
            Ok(())
        }

        async fn interrupt(&mut self) -> Result<()> {
            self.interrupts += 1;
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: self.supports_steer,
            }
        }

        fn provider_session_id(&self) -> Option<String> {
            Some("provider-session".to_string())
        }
    }

    async fn session(provider: &str) -> (SharedStore, ProjectSession) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep().join("registry.db");
        let store = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        let wave = Wave::new(
            LfdId::new(),
            format!("wave-{provider}"),
            "/repo".to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .unwrap();
        let session = ProjectSession {
            id: ProjectSessionId::new(),
            project: LinearProjectRef {
                id: LinearProjectId::new(format!("project-{provider}")).unwrap(),
                slug: "control".to_string(),
                name: "Control".to_string(),
                context: "Provider-neutral control".to_string(),
            },
            wave_id: wave.id().clone(),
            wave: wave.name().clone(),
            repo: "/repo".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
            status: ProjectSessionStatus::Running,
            status_reason: "provider active".to_string(),
            status_at: now,
            iteration: 0,
            task_event_cursor: 0,
            state_fingerprint: None,
            agent: provider.to_string(),
            provider: provider.to_string(),
            provider_session_id: Some("provider-session".to_string()),
            process: Some(ProjectProcess {
                generation: 1,
                pid: None,
                tmux_name: format!("project-{provider}"),
                started_at: now,
            }),
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&session).await.unwrap();
        (store, session)
    }

    #[tokio::test]
    async fn project_provider_control_reports_honest_steer_effects() {
        for (provider, supports_steer, expected_effect) in [
            ("codex", true, TaskCommandEffect::LiveSteer),
            ("claude", false, TaskCommandEffect::Replacement),
            ("opencode", false, TaskCommandEffect::Replacement),
        ] {
            let (store, session) = session(provider).await;
            let command = ProjectCommand::new(
                session.id.clone(),
                ProjectCommandSource::Human,
                ProjectCommandKind::Steer {
                    text: "change direction".to_string(),
                },
            );
            store.create_project_command(&command).await.unwrap();
            let commands = store.claim_project_commands(&session.id, 1).await.unwrap();
            let mut harness = ScriptedHarness {
                supports_steer,
                sent: Vec::new(),
                interrupts: 0,
            };
            let mut pending = std::collections::VecDeque::new();

            absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
                .await
                .unwrap();
            if let Some(input) = pending.pop_front() {
                apply_input(&store, &session, &mut harness, input)
                    .await
                    .unwrap();
            }

            let receipt = store
                .get_project_command(&command.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(receipt.state, TaskCommandState::Accepted, "{provider}");
            assert_eq!(receipt.effect, Some(expected_effect), "{provider}");
            assert_eq!(harness.sent, vec!["change direction"], "{provider}");
            assert_eq!(
                harness.interrupts,
                usize::from(!supports_steer),
                "{provider}"
            );
        }
    }

    #[tokio::test]
    async fn bare_project_interrupt_stops_without_abandoning_history() {
        let (store, session) = session("codex").await;
        let command = ProjectCommand::new(
            session.id.clone(),
            ProjectCommandSource::Human,
            ProjectCommandKind::Interrupt { replacement: None },
        );
        store.create_project_command(&command).await.unwrap();
        let commands = store.claim_project_commands(&session.id, 1).await.unwrap();
        let mut harness = ScriptedHarness {
            supports_steer: true,
            sent: Vec::new(),
            interrupts: 0,
        };
        let stop = absorb_commands(
            &store,
            &session,
            commands,
            &mut harness,
            true,
            &mut std::collections::VecDeque::new(),
        )
        .await
        .unwrap();

        assert!(matches!(stop, Some(CommandStop::Interrupted)));
        assert_eq!(harness.interrupts, 1);
        assert_eq!(
            store
                .get_project_command(&command.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            TaskCommandState::Accepted
        );
    }

    #[tokio::test]
    async fn project_decisions_resume_every_provider_without_waiting_for_the_blocked_turn() {
        for (provider, supports_steer) in [("codex", true), ("claude", false), ("opencode", false)]
        {
            let (store, session) = session(provider).await;
            let decision_id = ProjectDecisionId::new();
            let command = ProjectCommand::new(
                session.id.clone(),
                ProjectCommandSource::Human,
                ProjectCommandKind::Decide {
                    decision_id: decision_id.clone(),
                    choice: "approve".to_string(),
                    message: None,
                },
            );
            store.create_project_command(&command).await.unwrap();
            let commands = store.claim_project_commands(&session.id, 1).await.unwrap();
            let mut harness = ScriptedHarness {
                supports_steer,
                sent: Vec::new(),
                interrupts: 0,
            };
            let mut pending = std::collections::VecDeque::new();

            absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
                .await
                .unwrap();
            if let Some(input) = pending.pop_front() {
                apply_input(&store, &session, &mut harness, input)
                    .await
                    .unwrap();
            }

            assert_eq!(harness.interrupts, usize::from(!supports_steer));
            assert_eq!(
                harness.sent,
                vec![format!("Decision {decision_id}: approve")]
            );
            let receipt = store
                .get_project_command(&command.id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(receipt.state, TaskCommandState::Accepted);
            assert_eq!(receipt.effect, Some(TaskCommandEffect::Decision));
            assert!(store
                .project_events_after(&session.id, 0)
                .await
                .unwrap()
                .iter()
                .any(|event| matches!(
                    &event.kind,
                    crate::project_session::ProjectEventKind::DecisionResolved {
                        decision_id: resolved,
                        ..
                    } if resolved == &decision_id
                )));
        }
    }

    #[test]
    fn project_summary_is_bounded() {
        assert_eq!(
            super::bounded_summary(&"x".repeat(2_500)).chars().count(),
            2_000
        );
    }
}
