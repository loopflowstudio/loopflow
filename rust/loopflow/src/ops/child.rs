//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::child::{ChildBodyHandoffRequest, ChildRef};
use crate::durable::{
    AbandonReceipt, Author, ProjectChildControlToken, RunId, SteerReceipt, WorkRef,
    PROJECT_CHILD_CONTROL_ENV, RUN_ID_ENV,
};
use crate::store::SharedStore;
use crate::work::task::Task;

use super::{OpsError, OpsResult};

pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Steer { receipt: SteerReceipt },
    Resume { work: WorkRef },
    Abandon { receipt: AbandonReceipt },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Steer { receipt } => receipt.steer.id.to_string(),
            Self::Resume { work } => work.id().to_string(),
            Self::Abandon { receipt } => receipt.work.id().to_string(),
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Steer { .. } => "steered",
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
    authorize_task_resume(store, &task).await?;
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

pub(crate) async fn authorize_task_resume(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let run_id = std::env::var_os(RUN_ID_ENV)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| child_error("LF_RUN_ID is not valid UTF-8"))
                .and_then(|value| RunId::parse(&value).map_err(child_error))
        })
        .transpose()?;
    let token = std::env::var_os(PROJECT_CHILD_CONTROL_ENV)
        .map(|value| {
            value
                .into_string()
                .map_err(|_| child_error("Project child-control capability is not valid UTF-8"))
                .and_then(|value| ProjectChildControlToken::parse(&value).map_err(child_error))
        })
        .transpose()?;
    match (run_id, token) {
        (None, None) => Ok(()),
        (Some(run_id), Some(token)) => store
            .authorize_project_child_control(&task.id, &run_id, &token)
            .await
            .map(|_| ())
            .map_err(child_error),
        (Some(_), None) => Err(child_error(
            "in-Run Task resume has no Project child-control capability; restart the owning Project controller before pursuit",
        )),
        (None, Some(_)) => Err(child_error(
            "Project child-control capability has no exact Run identity; restart the owning Project controller",
        )),
    }
}

pub(crate) async fn append_steer(
    store: &SharedStore,
    target: ChildRef,
    text: &str,
) -> OpsResult<SteerReceipt> {
    let work = store.work_for_child(&target).await.map_err(child_error)?;
    let author = ambient_author()?;
    store
        .append_steer(&work, author, text)
        .await
        .map_err(child_error)
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
