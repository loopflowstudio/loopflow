use std::collections::{HashSet, VecDeque};
use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::task::{
    TaskCommand, TaskCommandId, TaskCommandKind, TaskEventKind, TaskSession, TaskSessionId,
    TaskSessionStatus,
};

struct PendingInput {
    command_id: Option<TaskCommandId>,
    text: String,
}

pub async fn run_task_session(session_id: TaskSessionId, generation: u32) -> Result<()> {
    let result = run_task_session_inner(session_id.clone(), generation).await;
    if let Err(error) = &result {
        record_unhandled_failure(&session_id, generation, error).await;
    }
    result
}

async fn run_task_session_inner(session_id: TaskSessionId, generation: u32) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let mut session = store
        .get_task_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {session_id} not found"))?;
    let recorded_generation = session.process.as_ref().map(|process| process.generation);
    if recorded_generation != Some(generation) {
        anyhow::bail!(
            "Task Session {session_id} generation mismatch: expected {:?}, got {generation}",
            recorded_generation
        );
    }

    if let Some(process) = &mut session.process {
        process.pid = Some(std::process::id());
    }
    let from = session.status;
    session.set_status(TaskSessionStatus::Running, "provider turn is active");
    store.update_task_session(&session).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Running,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    store
        .append_task_event(&session.id, &TaskEventKind::Started)
        .await?;

    let seed = task_seed(&session);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn("implement", &seed, &session.wave, None)?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let resuming = generation > 1 || session.provider_session_id.is_some();
    prepared.config.agent = Some(session.agent.clone());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    harness.start(&prepared.config).await?;
    if let Some(provider_session_id) = harness.provider_session_id() {
        session.provider_session_id = Some(provider_session_id);
    }
    session.provider = harness_name;
    store.update_task_session(&session).await?;

    let mut pending = VecDeque::new();
    let mut seen_commands = HashSet::new();
    let commands = store
        .claim_task_commands(&session.id, generation)
        .await?
        .into_iter()
        .filter(|command| seen_commands.insert(command.id.clone()))
        .collect();
    if let Some(reason) = absorb_commands(
        &store,
        &session,
        commands,
        harness.as_mut(),
        false,
        &mut pending,
    )
    .await?
    {
        return finish_abandoned(&store, &mut session, harness.as_mut(), reason).await;
    }
    let (first_input, first_command_id) = take_first_input(prepared.input, resuming, &mut pending);
    harness.send_input(&first_input).await?;
    if let Some(command_id) = first_command_id {
        store.acknowledge_task_command(&command_id).await?;
        record_command_accepted(&store, &session, command_id).await?;
    }

    let (attachment_tx, mut attachment_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if attachment_tx.send(line).is_err() {
                break;
            }
        }
    });
    println!(
        "task {}> attached; /status, /interrupt [message], /detach, or type an instruction",
        session.issue.identifier
    );
    let mut command_poll = tokio::time::interval(Duration::from_millis(200));
    command_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &session, line).await?;
                }
            }
            _ = command_poll.tick() => {
                let commands = store
                    .claim_task_commands(&session.id, generation)
                    .await?
                    .into_iter()
                    .filter(|command| seen_commands.insert(command.id.clone()))
                    .collect();
                if let Some(reason) = absorb_commands(
                    &store,
                    &session,
                    commands,
                    harness.as_mut(),
                    true,
                    &mut pending,
                ).await? {
                    return finish_abandoned(&store, &mut session, harness.as_mut(), reason).await;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut session,
                        harness.as_mut(),
                        "provider event stream closed",
                    ).await;
                };
                if session.provider_session_id.is_none() {
                    if let Some(provider_session_id) = harness.provider_session_id() {
                        session.provider_session_id = Some(provider_session_id);
                        store.update_task_session(&session).await?;
                    }
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            return finish_failed(
                                &store,
                                &mut session,
                                harness.as_mut(),
                                "provider turn failed",
                            ).await;
                        }
                        if let Some(input) = pending.pop_front() {
                            harness.send_input(&input.text).await?;
                            if let Some(command_id) = input.command_id {
                                store.acknowledge_task_command(&command_id).await?;
                                record_command_accepted(&store, &session, command_id).await?;
                            }
                            continue;
                        }
                        let summary = progress_summary(&last_text);
                        if !summary.is_empty() {
                            store.append_task_event(
                                &session.id,
                                &TaskEventKind::Progress {
                                    summary: summary.clone(),
                                },
                            ).await?;
                        }
                        let _ = harness.stop().await;
                        let from = session.status;
                        let observed_pr = crate::ops::current_or_merged_pr(&session.worktree).ok().flatten();
                        if let Some(pr) = observed_pr {
                            let pull_request = crate::task::PullRequestRef {
                                number: pr.number as u32,
                                url: pr.url.clone(),
                            };
                            session.pull_request = Some(pull_request.clone());
                            if pr.state == "merged" {
                                session.set_status(
                                    TaskSessionStatus::Merged,
                                    format!("pull request #{} merged", pr.number),
                                );
                                crate::ops::task::reconcile_pm_writeback(&mut session).await;
                                store.append_task_event(
                                    &session.id,
                                    &TaskEventKind::Completed {
                                        pull_request,
                                        summary,
                                    },
                                ).await?;
                            } else {
                                session.set_status(
                                    TaskSessionStatus::Submitted,
                                    format!("pull request #{} is open for review", pr.number),
                                );
                                store.append_task_event(
                                    &session.id,
                                    &TaskEventKind::PullRequestOpened {
                                        number: pr.number as u32,
                                        url: pr.url,
                                    },
                                ).await?;
                            }
                        } else {
                            session.set_status(
                                TaskSessionStatus::Waiting,
                                "provider turn completed; Task Session is waiting for review, merge, or another instruction",
                            );
                        }
                        store.update_task_session(&session).await?;
                        store.append_task_event(
                            &session.id,
                            &TaskEventKind::StatusChanged {
                                from,
                                to: session.status,
                                reason: session.status_reason.clone(),
                            },
                        ).await?;
                        crate::lf::commands::chat::post_to_named_wave(
                            &session.wave,
                            &format!(
                                "Task {} → {}: {}",
                                session.issue.identifier,
                                session.status.as_str(),
                                session.status_reason
                            ),
                        ).await?;
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

fn take_first_input(
    task_seed: String,
    resuming: bool,
    pending: &mut VecDeque<PendingInput>,
) -> (String, Option<TaskCommandId>) {
    if !resuming {
        return (task_seed, None);
    }
    pending
        .pop_front()
        .map_or((task_seed, None), |input| (input.text, input.command_id))
}

async fn handle_attachment(store: &SharedStore, session: &TaskSession, line: String) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        println!(
            "{}  {}  {}",
            session.issue.identifier,
            session.status.as_str(),
            session.status_reason
        );
        return Ok(());
    }
    if line == "/detach" {
        let _ = std::process::Command::new("tmux")
            .args(["detach-client"])
            .status();
        return Ok(());
    }
    let kind = if let Some(message) = line.strip_prefix("/interrupt") {
        let message = message.trim();
        TaskCommandKind::Interrupt {
            next_message: (!message.is_empty()).then(|| message.to_string()),
        }
    } else {
        TaskCommandKind::Message {
            text: line.to_string(),
        }
    };
    let command = TaskCommand::new(
        session.id.clone(),
        crate::task::TaskCommandSource::Attachment,
        kind,
    );
    store.create_task_command(&command).await?;
    crate::lf::commands::chat::post_to_named_wave(
        &session.wave,
        &format!(
            "Task command {} → {} (attachment)",
            command.id, session.issue.identifier
        ),
    )
    .await?;
    println!("queued {}", command.id);
    Ok(())
}

async fn absorb_commands(
    store: &SharedStore,
    session: &TaskSession,
    commands: Vec<TaskCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<String>> {
    for command in commands {
        match command.kind {
            TaskCommandKind::Message { text }
            | TaskCommandKind::Resume {
                message: Some(text),
            } => {
                if turn_active && harness.capabilities().supports_steer {
                    harness.send_input(&text).await?;
                    store.acknowledge_task_command(&command.id).await?;
                    record_command_accepted(store, session, command.id).await?;
                } else {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                    });
                }
            }
            TaskCommandKind::Resume { message: None } => {
                store.acknowledge_task_command(&command.id).await?;
                record_command_accepted(store, session, command.id).await?;
            }
            TaskCommandKind::Interrupt { next_message } => {
                if turn_active {
                    harness.interrupt().await?;
                }
                if let Some(text) = next_message {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                    });
                } else {
                    store.acknowledge_task_command(&command.id).await?;
                    record_command_accepted(store, session, command.id).await?;
                }
            }
            TaskCommandKind::Abandon { reason } => {
                store.acknowledge_task_command(&command.id).await?;
                record_command_accepted(store, session, command.id).await?;
                return Ok(Some(reason));
            }
        }
    }
    Ok(None)
}

async fn record_unhandled_failure(
    session_id: &TaskSessionId,
    generation: u32,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_task_session(session_id).await else {
        return;
    };
    if !session.status.is_process_active()
        || session.process.as_ref().map(|process| process.generation) != Some(generation)
    {
        return;
    }
    let from = session.status;
    let message = format!("task process failed: {error}");
    session.set_status(TaskSessionStatus::Failed, &message);
    if store.update_task_session(&session).await.is_err() {
        return;
    }
    let _ = store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: message.clone(),
            },
        )
        .await;
    let _ = store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error: message.clone(),
                resumable: true,
            },
        )
        .await;
    let _ = crate::lf::commands::chat::post_to_named_wave(
        &session.wave,
        &format!("Task {} → failed: {message}", session.issue.identifier),
    )
    .await;
}

async fn record_command_accepted(
    store: &SharedStore,
    session: &TaskSession,
    command_id: TaskCommandId,
) -> Result<()> {
    store
        .append_task_event(&session.id, &TaskEventKind::CommandAccepted { command_id })
        .await?;
    Ok(())
}

async fn finish_failed(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    error: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    let from = session.status;
    session.set_status(TaskSessionStatus::Failed, error);
    store.update_task_session(session).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
    anyhow::bail!(error.to_string())
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    reason: String,
) -> Result<()> {
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    let from = session.status;
    session.set_status(
        TaskSessionStatus::Abandoned,
        format!("Task Session explicitly abandoned: {reason}"),
    );
    store.update_task_session(session).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Abandoned,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    Ok(())
}

fn task_seed(session: &TaskSession) -> String {
    let snapshot_warning = session
        .pm_snapshot_warning
        .as_deref()
        .map(|warning| format!("\nPM snapshot warning: {warning}"))
        .unwrap_or_default();
    format!(
        "Implement Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\nPM snapshot synced at: {snapshot_synced_at}{snapshot_warning}\nWave: {wave}\nTask Session: {session_id}\nWorktree: {worktree}\nBase commit: {base_commit}\nDelivery: one pull request targeting main. Opening the PR submits the task; completion is merge or explicit abandonment.",
        identifier = session.issue.identifier,
        title = session.issue.title,
        description = session.issue.description,
        project = session.project.name,
        project_id = session.project.id.as_str(),
        project_context = session.project.context,
        snapshot_synced_at = session.pm_snapshot_synced_at,
        wave = session.wave,
        session_id = session.id,
        worktree = session.worktree.display(),
        base_commit = session.base_commit,
    )
}

fn progress_summary(text: &str) -> String {
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
    use std::collections::VecDeque;

    use super::{progress_summary, take_first_input, PendingInput};
    use crate::task::TaskCommandId;

    #[test]
    fn progress_summary_bounds_wave_visible_text() {
        let summary = progress_summary(&"x".repeat(2_500));
        assert_eq!(summary.chars().count(), 2_000);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn fresh_session_keeps_task_seed_ahead_of_racing_commands() {
        let command_id = TaskCommandId::new();
        let mut pending = VecDeque::from([PendingInput {
            command_id: Some(command_id.clone()),
            text: "rename the flag".to_string(),
        }]);

        let first = take_first_input("implement INF-123".to_string(), false, &mut pending);

        assert_eq!(first, ("implement INF-123".to_string(), None));
        assert_eq!(
            pending.front().and_then(|input| input.command_id.clone()),
            Some(command_id)
        );
    }

    #[test]
    fn resumed_session_starts_with_the_oldest_queued_command() {
        let command_id = TaskCommandId::new();
        let mut pending = VecDeque::from([PendingInput {
            command_id: Some(command_id.clone()),
            text: "address review".to_string(),
        }]);

        let first = take_first_input("original seed".to_string(), true, &mut pending);

        assert_eq!(first, ("address review".to_string(), Some(command_id)));
        assert!(pending.is_empty());
    }
}
