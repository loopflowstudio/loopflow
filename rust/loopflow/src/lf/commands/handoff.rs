use std::collections::BTreeMap;
use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{anyhow, Context};
use serde::Serialize;

use crate::engine::wave_home::WaveHome;
use crate::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffAttach, InteractiveHandoffId, InteractiveHandoffOutcome,
    InteractiveHandoffParent, OpenInteractiveHandoff,
};
use crate::lf::HandoffCommand;
use crate::store::{open_store, storage_config_from_env, Store};
use crate::task::TaskSessionId;

#[derive(Debug, Serialize)]
struct OpenResult {
    created: bool,
    handoff: InteractiveHandoff,
}

pub fn run(command: &HandoffCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(command))
}

async fn run_async(command: &HandoffCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        HandoffCommand::Open {
            parent,
            home,
            cwd,
            provider,
            provider_session,
            generation,
            reason,
            environment,
            json,
            attach_argv,
        } => {
            let request = OpenInteractiveHandoff {
                parent: parent.parse()?,
                home: home.parse::<WaveHome>().map_err(|error| anyhow!(error))?,
                cwd: cwd.clone(),
                provider: provider.clone(),
                provider_session_id: provider_session.clone(),
                body_generation: *generation,
                reason: reason.clone(),
                environment: parse_environment(environment)?,
                attach_argv: attach_argv.clone(),
            };
            let (handoff, created) = store.open_interactive_handoff(request).await?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&OpenResult { created, handoff })?
                );
            } else {
                let action = if created { "opened" } else { "reused" };
                println!(
                    "{action} handoff {} for {} ({})",
                    handoff.id,
                    handoff.parent,
                    handoff.status.as_str()
                );
            }
        }
        HandoffCommand::List {
            active,
            parent,
            json,
        } => {
            let parent = parent
                .as_deref()
                .map(InteractiveHandoffParent::parse)
                .transpose()?;
            let mut handoffs = store.list_interactive_handoffs(parent.as_ref()).await?;
            if *active {
                handoffs.retain(InteractiveHandoff::is_active);
            }
            let now = time::OffsetDateTime::now_utc();
            let rows: Vec<_> = handoffs
                .iter()
                .map(|handoff| handoff.list_row(now))
                .collect();
            if *json {
                println!("{}", serde_json::to_string_pretty(&rows)?);
            } else if rows.is_empty() {
                println!("no interactive handoffs");
            } else {
                for row in &rows {
                    println!(
                        "{} {}:{} ({}) — {}",
                        row.session_id,
                        row.parent_kind,
                        row.parent_id,
                        row.status.as_str(),
                        row.reason
                    );
                }
            }
        }
        HandoffCommand::Status { session_id, json } => {
            let handoff = load_handoff(&store, session_id).await?;
            print_handoff(&handoff, *json)?;
        }
        HandoffCommand::Attach { session_id, json } => {
            let session_id = parse_session_id(session_id)?;
            let handoff = store.attach_interactive_handoff(&session_id).await?;
            print_attach(&handoff.attach_descriptor(), *json)?;
        }
        HandoffCommand::Complete {
            session_id,
            summary,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::Completed {
                    summary: summary.clone(),
                },
                *json,
            )
            .await?;
        }
        HandoffCommand::Back {
            session_id,
            summary,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::HandedBack {
                    summary: summary.clone(),
                },
                *json,
            )
            .await?;
        }
        HandoffCommand::Fail {
            session_id,
            reason,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::Failed {
                    reason: reason.clone(),
                },
                *json,
            )
            .await?;
        }
        HandoffCommand::Present { session_id } => {
            let session_id = parse_session_id(session_id)?;
            let handoff = store.attach_interactive_handoff(&session_id).await?;
            let descriptor = handoff.attach_descriptor();
            present(&descriptor)?;
        }
    }
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Store> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .context("open the shared Loopflow store")
}

async fn load_handoff(store: &Store, session_id: &str) -> anyhow::Result<InteractiveHandoff> {
    let session_id = parse_session_id(session_id)?;
    store
        .get_interactive_handoff(&session_id)
        .await?
        .ok_or_else(|| anyhow!("interactive handoff {session_id} not found"))
}

async fn finish(
    store: &Store,
    session_id: &str,
    outcome: InteractiveHandoffOutcome,
    json: bool,
) -> anyhow::Result<()> {
    let session_id = parse_session_id(session_id)?;
    let handoff = store
        .finish_interactive_handoff(&session_id, &outcome)
        .await?;
    wake_parent(store, &handoff).await;
    print_handoff(&handoff, json)
}

/// Wake the blocked parent so a fresh body reconciles this terminal outcome and
/// resumes exactly once (the wake-claim guard makes duplicate resumes harmless).
/// Best-effort: the handoff is already durably recorded, so a resume that cannot
/// be queued — parent abandoned, registry busy — must not fail the human's
/// completion. Project and Wave parents resume through their own supervision.
async fn wake_parent(store: &Store, handoff: &InteractiveHandoff) {
    let InteractiveHandoffParent::Task(task_id) = &handoff.parent else {
        return;
    };
    if let Err(error) = resume_task_parent(store, task_id, handoff.outcome.as_ref()).await {
        eprintln!(
            "warning: interactive handoff {} resolved but waking parent Task {} failed: {error}",
            handoff.id, task_id
        );
    }
}

async fn resume_task_parent(
    store: &Store,
    task_id: &TaskSessionId,
    outcome: Option<&InteractiveHandoffOutcome>,
) -> anyhow::Result<()> {
    let session = store
        .get_task_session(task_id)
        .await?
        .ok_or_else(|| anyhow!("parent Task Session {task_id} not found"))?;
    if let Some(message) = handoff_resume_message(outcome) {
        let work = store
            .work_for_child(&crate::child_session::ChildRef::Task(session.id.clone()))
            .await?;
        store
            .append_steer(&work, crate::durable::Author::User, &message, None)
            .await?;
    }
    crate::ops::task::resume_task_async(
        &session.launch.issue.identifier,
        None,
        Some("interactive handoff resolved".to_string()),
    )
    .await
    .map_err(|error| anyhow!(error.to_string()))?;
    Ok(())
}

fn handoff_resume_message(outcome: Option<&InteractiveHandoffOutcome>) -> Option<String> {
    match outcome {
        Some(InteractiveHandoffOutcome::HandedBack { summary }) => Some(format!(
            "Resume the interrupted Task step after the human handed back the interactive work:\n{summary}"
        )),
        Some(
            InteractiveHandoffOutcome::Completed { .. }
            | InteractiveHandoffOutcome::Failed { .. },
        )
        | None => None,
    }
}

fn parse_session_id(value: &str) -> anyhow::Result<InteractiveHandoffId> {
    InteractiveHandoffId::parse(value).map_err(Into::into)
}

fn parse_environment(values: &[String]) -> anyhow::Result<BTreeMap<String, String>> {
    let mut environment = BTreeMap::new();
    for value in values {
        let (key, entry) = value
            .split_once('=')
            .ok_or_else(|| anyhow!("environment entry {value:?} must be KEY=VALUE"))?;
        if key.trim().is_empty() {
            return Err(anyhow!("environment key cannot be empty"));
        }
        if environment
            .insert(key.to_string(), entry.to_string())
            .is_some()
        {
            return Err(anyhow!("duplicate environment key {key:?}"));
        }
    }
    Ok(environment)
}

fn print_handoff(handoff: &InteractiveHandoff, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(handoff)?);
    } else {
        println!(
            "{}  {}\n  parent: {}\n  provider: {}\n  cwd: {}\n  reason: {}",
            handoff.id,
            handoff.status.as_str(),
            handoff.parent,
            handoff.provider,
            handoff.cwd.display(),
            handoff.reason
        );
    }
    Ok(())
}

fn print_attach(attach: &InteractiveHandoffAttach, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(attach)?);
    } else {
        println!(
            "{}  {}\n  host: {}\n  cwd: {}\n  env: {}\n  argv: {}",
            attach.session_id,
            attach.status.as_str(),
            attach.host,
            attach.cwd.display(),
            serde_json::to_string(&attach.environment)?,
            serde_json::to_string(&attach.argv)?,
        );
    }
    Ok(())
}

/// CLI presentation adapter: attach and exec into the interactive terminal.
///
/// Records first-attach evidence, then replaces this process with the terminal
/// session (e.g. `tmux attach-session`). When the terminal exits, control
/// returns to the caller's shell — the human can then complete, hand back, or
/// fail the handoff.
fn present(descriptor: &InteractiveHandoffAttach) -> anyhow::Result<()> {
    let argv: Vec<&str> = descriptor.argv.iter().map(String::as_str).collect();
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("attach argv is empty"))?;

    let mut command = Command::new(program);
    command.args(args);
    command.current_dir(&descriptor.cwd);
    for (key, value) in &descriptor.environment {
        command.env(key, value);
    }

    // exec replaces the process — the terminal session inherits our stdin/stdout/stderr.
    let error = command.exec();
    Err(anyhow!("failed to exec {program:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{handoff_resume_message, parse_environment};
    use crate::interactive_handoff::InteractiveHandoffOutcome;

    #[test]
    fn environment_parser_preserves_values_without_building_shell_text() {
        let parsed =
            parse_environment(&["LF_HOME=/tmp/lf".to_string(), "EMPTY=".to_string()]).unwrap();
        assert_eq!(parsed.get("LF_HOME").map(String::as_str), Some("/tmp/lf"));
        assert_eq!(parsed.get("EMPTY").map(String::as_str), Some(""));
        assert!(parse_environment(&["LF_HOME=/a".to_string(), "LF_HOME=/b".to_string()]).is_err());
    }

    #[test]
    fn hand_back_resumes_the_same_task_step_with_the_human_summary() {
        let message = handoff_resume_message(Some(&InteractiveHandoffOutcome::HandedBack {
            summary: "Finish the review fixes headlessly".to_string(),
        }))
        .unwrap();
        assert!(message.contains("Resume the interrupted Task step"));
        assert!(message.contains("Finish the review fixes headlessly"));
        assert!(
            handoff_resume_message(Some(&InteractiveHandoffOutcome::Completed {
                summary: "Human finished the whole step".to_string(),
            }))
            .is_none()
        );
    }
}
