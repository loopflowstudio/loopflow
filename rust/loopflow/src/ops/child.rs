//! Shared recovery and execution helpers for Project and Task Work.

use crate::child::{ChildBodyHandoffRequest, ChildRef};
use crate::durable::{AbandonReceipt, Author, RunId, Steer, WorkRef, RUN_ID_ENV};
use crate::store::SharedStore;
use crate::work::task::Task;

use super::{OpsError, OpsResult};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Steer { steer: Steer },
    Interrupt { work: WorkRef },
    Resume { work: WorkRef },
    Abandon { receipt: AbandonReceipt },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Steer { steer } => steer.id.to_string(),
            Self::Interrupt { work } => work.id().to_string(),
            Self::Resume { work } => work.id().to_string(),
            Self::Abandon { receipt } => receipt.work.id().to_string(),
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Steer { .. } => "steered",
            Self::Interrupt { .. } => "interrupted",
            Self::Resume { .. } => "resumed",
            Self::Abandon { .. } => "abandoned",
        }
    }
}

pub(crate) async fn resume_task(
    store: &SharedStore,
    mut task: Task,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<WorkRef> {
    let mut controller = store
        .task_controller_state(&task.id)
        .await
        .map_err(child_error)?
        .ok_or_else(|| {
            child_error(format!(
                "Task {} has no end-to-end controller; use `lf task run {}` to install one",
                task.plan.identifier, task.plan.identifier
            ))
        })?;
    if let Some(model) = model {
        let request = handoff_request(&model, reason.as_deref())?;
        if controller.agent != request.agent {
            controller = store
                .handoff_task_controller(&task.id, &request)
                .await
                .map_err(child_error)?;
        }
    }
    let _ = controller;
    let label = format!("Task {}", task.plan.identifier);
    if let Some(intent) = &task.abandon_intent {
        return Err(child_error(format!(
            "{label} is being abandoned: {}",
            intent.reason
        )));
    }
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(child_error)?;
    super::task::resume_inactive_process(store, &mut task).await?;
    Ok(work)
}

pub(crate) async fn append_steer(
    store: &SharedStore,
    target: ChildRef,
    text: &str,
) -> OpsResult<Steer> {
    let work = store.work_for_child(&target).await.map_err(child_error)?;
    let author = ambient_author()?;
    store
        .append_steer(&work, author, text)
        .await
        .map_err(child_error)
}

/// Inject steer comments newer than `*cursor` into the live provider turn. A
/// comment the provider takes (`Sent`) advances the cursor; one it can't take
/// right now (`NotSteerable` — no active turn, or a driver without live input)
/// stays for the next skill boundary, whose seed re-reads every steer. Steering
/// is best-effort live and durable at the boundary, so this never fails the run.
pub(crate) async fn inject_live_steers(
    store: &SharedStore,
    work: &WorkRef,
    harness: &mut dyn crate::harness::Harness,
    cursor: &mut i64,
) {
    let steers = match store.work_steers(work).await {
        Ok(steers) => steers,
        Err(error) => {
            tracing::warn!(%error, "failed to read steers for live injection");
            return;
        }
    };
    for steer in &steers {
        if steer.id <= *cursor {
            continue;
        }
        match harness.send_current(&steer.text).await {
            crate::harness::SendCurrentOutcome::Sent { .. } => *cursor = steer.id,
            crate::harness::SendCurrentOutcome::NotSteerable => break,
            crate::harness::SendCurrentOutcome::Failed { error }
            | crate::harness::SendCurrentOutcome::Unknown { error, .. } => {
                tracing::warn!(
                    %error,
                    steer = steer.id,
                    "live steer injection failed; deferring to the next boundary"
                );
                break;
            }
        }
    }
}

/// End the current turn if an interrupt was requested after `*cursor`. The
/// interrupt is a one-time durable event, so the cursor only advances (a turn
/// boundary never resets it) — a request is acted on exactly once per run.
pub(crate) async fn observe_interrupt(
    store: &SharedStore,
    work: &WorkRef,
    harness: &mut dyn crate::harness::Harness,
    cursor: &mut i64,
) {
    match store.latest_interrupt_id(work).await {
        Ok(id) if id > *cursor => {
            *cursor = id;
            if let Err(error) = harness.interrupt().await {
                tracing::warn!(%error, "interrupt request failed to reach the provider");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "failed to read interrupt requests"),
    }
}

pub(crate) fn ambient_author() -> OpsResult<Author> {
    Ok(std::env::var_os(RUN_ID_ENV)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| child_error("LF_RUN_ID is not valid UTF-8"))
                .and_then(|value| RunId::parse(&value).map_err(child_error))
                .map(Author::Run)
        })
        .transpose()?
        .unwrap_or(Author::User))
}

pub(crate) fn handoff_request(
    model: &str,
    reason: Option<&str>,
) -> OpsResult<ChildBodyHandoffRequest> {
    let agent = model.trim();
    if agent.is_empty() {
        return Err(child_error("handoff model cannot be empty"));
    }
    let (provider, model_name) = agent
        .split_once(':')
        .map_or((agent, None), |(provider, model_name)| {
            (provider, Some(model_name))
        });
    let provider = crate::harness::canonical_harness(provider)
        .ok_or_else(|| child_error(format!("unsupported provider harness: {provider}")))?;
    let agent = match model_name {
        Some(model_name) if model_name.trim().is_empty() => {
            return Err(child_error("handoff model name cannot be empty"));
        }
        Some(model_name) => format!("{provider}:{}", model_name.trim()),
        None => provider.to_string(),
    };
    let reason = reason
        .unwrap_or("operator requested provider handoff")
        .trim();
    if reason.is_empty() {
        return Err(child_error("handoff reason cannot be empty"));
    }
    Ok(ChildBodyHandoffRequest {
        agent,
        provider: provider.to_string(),
        reason: reason.to_string(),
    })
}

fn child_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_run_id_is_opaque_steer_authorship() {
        let _lock = crate::journal::test_env_lock();
        let previous_run_id = std::env::var_os(RUN_ID_ENV);
        let run_id = RunId::new();
        std::env::set_var(RUN_ID_ENV, run_id.as_str());

        let author = ambient_author().unwrap();

        match previous_run_id {
            Some(value) => std::env::set_var(RUN_ID_ENV, value),
            None => std::env::remove_var(RUN_ID_ENV),
        }
        assert_eq!(author, Author::Run(run_id));
    }
}
