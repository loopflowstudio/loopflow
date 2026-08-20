//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::child::{ChildBodyHandoffRequest, ChildRef};
use crate::durable::{
    AuthenticatedRequest, ControlCtx, EpochReceipt, InterruptReceipt, Run, RunLease, RunLeaseToken,
    SteerReceipt, RUN_CONTEXT_ENV, RUN_LEASE_ENV,
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
    if let Some(lease) = ambient_run_lease(store).await? {
        store
            .steer(&ControlCtx::Run(&lease), &work, text, None)
            .await
            .map_err(child_error)
    } else {
        let request = AuthenticatedRequest::cli();
        store
            .steer(&ControlCtx::User(&request), &work, text, None)
            .await
            .map_err(child_error)
    }
}

pub async fn ambient_run_lease(store: &Store) -> OpsResult<Option<RunLease>> {
    if let Some(value) = std::env::var_os(RUN_LEASE_ENV) {
        let value = value
            .into_string()
            .map_err(|_| child_error("LF_RUN_LEASE is not valid UTF-8"))?;
        let token =
            RunLeaseToken::parse(&value).map_err(|_| child_error("LF_RUN_LEASE is malformed"))?;
        return store
            .resolve_run_lease(token)
            .await
            .map(Some)
            .map_err(child_error);
    }

    // `LF_RUN_CONTEXT` is the sole positive marker that this process is an
    // agent body rather than a person at a shell. It matters because User
    // authority is the ambient fallback: `AuthenticatedRequest::cli()` treats
    // local shell presence as the user, which is right for a local-first
    // product but means a body that lost its lease would otherwise inherit
    // full User rights. Every Run body sets this var, so its presence without a
    // resolvable lease is a fenced or stale writer and must fail closed.
    //
    // This deliberately does not consult the deleted Task/Project executor env
    // vars. They served as this sentinel before Run owned authority; keying on
    // them now would make deleting them a silent privilege escalation.
    if std::env::var_os(RUN_CONTEXT_ENV).is_some() {
        return Err(child_error(
            "in-Run agent process has no usable LF_RUN_LEASE; refusing User authority",
        ));
    }
    Ok(None)
}

pub(crate) async fn required_run_lease(store: &Store) -> OpsResult<RunLease> {
    ambient_run_lease(store).await?.ok_or_else(|| {
        child_error(
            "this Work-owned entrypoint requires a Run lease; launch a named skill with \
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
    fn run_context_without_a_lease_fails_closed() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let store = runtime
            .block_on(open_store(&StorageConfig::sqlite(
                directory.path().join("registry.db"),
            )))
            .unwrap();
        let _lock = crate::journal::test_env_lock();
        let previous_context = std::env::var_os(RUN_CONTEXT_ENV);
        let previous_lease = std::env::var_os(RUN_LEASE_ENV);
        std::env::set_var(RUN_CONTEXT_ENV, "agent");
        std::env::remove_var(RUN_LEASE_ENV);

        let error = runtime
            .block_on(ambient_run_lease(&store))
            .expect_err("stale agent context must not inherit User authority");

        match previous_context {
            Some(value) => std::env::set_var(RUN_CONTEXT_ENV, value),
            None => std::env::remove_var(RUN_CONTEXT_ENV),
        }
        match previous_lease {
            Some(value) => std::env::set_var(RUN_LEASE_ENV, value),
            None => std::env::remove_var(RUN_LEASE_ENV),
        }
        assert!(error.to_string().contains("refusing User authority"));
    }
}
