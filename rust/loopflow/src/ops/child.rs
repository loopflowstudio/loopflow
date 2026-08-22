//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::child::{ChildBodyHandoffRequest, ChildRef};
use crate::durable::{
    EpochReceipt, InterruptReceipt, Run, RunContext, RunId, SteerReceipt, RUN_CONTEXT_ENV,
    RUN_ID_ENV,
};
use crate::project::Project;
use crate::store::{SharedStore, Store};
use crate::task::Task;

use super::{OpsError, OpsResult};

pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Steer { receipt: SteerReceipt },
    Interrupt { receipt: InterruptReceipt },
    Resume { run: Run },
    Abandon { receipt: EpochReceipt },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Steer { receipt } => receipt.steer.id.to_string(),
            Self::Interrupt { receipt } => receipt.run_id.to_string(),
            Self::Resume { run } => run.id.to_string(),
            Self::Abandon { receipt } => receipt.epoch.id.to_string(),
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

#[derive(Debug)]
pub(crate) enum Child {
    Project(Box<Project>),
    Task(Box<Task>),
}

impl Child {
    fn target(&self) -> ChildRef {
        match self {
            Self::Project(project) => ChildRef::Project(project.id.clone()),
            Self::Task(task) => ChildRef::Task(task.id.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Project(project) => format!("Project {}", project.plan.slug),
            Self::Task(task) => format!("Task {}", task.plan.identifier),
        }
    }

    fn agent(&self) -> &str {
        match self {
            Self::Project(project) => &project.agent,
            Self::Task(task) => &task.agent,
        }
    }

    async fn handoff(
        &mut self,
        store: &SharedStore,
        request: &ChildBodyHandoffRequest,
    ) -> OpsResult<()> {
        *self = match self {
            Self::Project(project) => Self::Project(Box::new(
                store
                    .handoff_project_body(&project.id, request)
                    .await
                    .map_err(child_error)?,
            )),
            Self::Task(task) => Self::Task(Box::new(
                store
                    .handoff_task_body(&task.id, request)
                    .await
                    .map_err(child_error)?,
            )),
        };
        Ok(())
    }

    fn abandon_intent_reason(&self) -> Option<String> {
        let intent = match self {
            Self::Project(project) => project.abandon_intent.as_ref(),
            Self::Task(task) => task.abandon_intent.as_ref(),
        };
        intent.map(|intent| {
            format!(
                "{} is being abandoned: {}",
                self.label(),
                intent.reason.clone()
            )
        })
    }

    async fn launch(&mut self, store: &SharedStore) -> OpsResult<()> {
        if let Some(bar) = self.abandon_intent_reason() {
            return Err(child_error(bar));
        }
        match self {
            Self::Project(project) => super::project::launch_project_process(store, project).await,
            Self::Task(task) => super::task::resume_inactive_process(store, task).await,
        }
    }
}

pub(crate) async fn resume_child(
    store: &SharedStore,
    mut child: Child,
    model: Option<String>,
    reason: Option<String>,
) -> OpsResult<Run> {
    if let Some(model) = model {
        let request = handoff_request(&model, reason.as_deref())?;
        if child.agent() != request.agent {
            child.handoff(store, &request).await?;
        }
    }
    let work = store
        .work_for_child(&child.target())
        .await
        .map_err(child_error)?;
    if !matches!(
        store.work_status(&work).await.map_err(child_error)?,
        crate::durable::WorkStatus::Running { .. }
    ) {
        child.launch(store).await?;
    }
    store
        .current_run(&work)
        .await
        .map_err(child_error)?
        .ok_or_else(|| child_error(format!("{} has no active Run after resume", child.label())))
}

pub(crate) async fn append_steer(
    store: &SharedStore,
    target: ChildRef,
    text: &str,
) -> OpsResult<SteerReceipt> {
    let work = store.work_for_child(&target).await.map_err(child_error)?;
    if let Some(lease) = ambient_run_context(store).await? {
        store
            .steer(Some(&lease), &work, text, None)
            .await
            .map_err(child_error)
    } else {
        store
            .steer(None, &work, text, None)
            .await
            .map_err(child_error)
    }
}

pub async fn ambient_run_context(store: &Store) -> OpsResult<Option<RunContext>> {
    if let Some(value) = std::env::var_os(RUN_ID_ENV) {
        let value = value
            .into_string()
            .map_err(|_| child_error("LF_RUN_ID is not valid UTF-8"))?;
        let run_id = RunId::parse(&value).map_err(|_| child_error("LF_RUN_ID is malformed"))?;
        return store
            .run_context(&run_id)
            .await
            .map(Some)
            .map_err(child_error);
    }
    if std::env::var_os(RUN_CONTEXT_ENV).is_some() {
        return Err(child_error(
            "in-Run agent process has no LF_RUN_ID; refusing an unattributed write",
        ));
    }
    Ok(None)
}

pub(crate) async fn required_run_context(store: &Store) -> OpsResult<RunContext> {
    ambient_run_context(store).await?.ok_or_else(|| {
        child_error(
            "this Work-owned entrypoint requires a Run id; launch a named skill with \
             `lf --as task:<selector> <skill>` (or project:/wave:)",
        )
    })
}

fn handoff_request(model: &str, reason: Option<&str>) -> OpsResult<ChildBodyHandoffRequest> {
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
    use crate::store::{open_store, StorageConfig};

    #[test]
    fn agent_process_without_a_run_id_fails_provenance_validation() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let store = runtime
            .block_on(open_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            )))
            .unwrap();
        let _lock = crate::journal::test_env_lock();
        let previous_context = std::env::var_os(RUN_CONTEXT_ENV);
        let previous_run_id = std::env::var_os(RUN_ID_ENV);
        std::env::set_var(RUN_CONTEXT_ENV, "agent");
        std::env::remove_var(RUN_ID_ENV);

        let error = runtime
            .block_on(ambient_run_context(&store))
            .expect_err("an agent body must name its Run generation");

        match previous_context {
            Some(value) => std::env::set_var(RUN_CONTEXT_ENV, value),
            None => std::env::remove_var(RUN_CONTEXT_ENV),
        }
        match previous_run_id {
            Some(value) => std::env::set_var(RUN_ID_ENV, value),
            None => std::env::remove_var(RUN_ID_ENV),
        }
        assert!(error.to_string().contains("refusing an unattributed write"));
    }
}
