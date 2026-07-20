use std::path::PathBuf;

use crate::durable::{
    AdvanceReceipt, AgentInvocation, BoundaryState, Containment, InvocationRoute, RunAdvance,
    RunLease, WorkRef,
};
use crate::engine::process::{
    current_home_execution_context, pin_control_binary, start_lf_session_with_env,
};
use crate::id::WaveId;
use crate::store::SharedStore;

use super::{OpsError, OpsResult};

#[derive(Debug)]
pub(crate) struct RunLaunch {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub agent: String,
    pub account_id: Option<crate::store::ProviderAccountId>,
    pub resume_token: Option<String>,
}

pub(crate) async fn launch_in_run(
    store: &SharedStore,
    lease: &RunLease,
    request: RunLaunch,
) -> OpsResult<AgentInvocation> {
    let execution = current_home_execution_context()
        .map_err(|error| OpsError::Message(format!("cannot resolve current lf binary: {error}")))?;
    let (provider, model) = crate::engine::config::parse_agent(&request.agent);
    let tmux_name = request.tmux_name.clone();
    let run = store
        .advance_run(
            lease,
            RunAdvance::RunStarting {
                containment: Containment::Tmux {
                    name: tmux_name.clone(),
                },
                cwd: request.cwd.clone(),
            },
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let AdvanceReceipt::Run(_) = run else {
        unreachable!("RunStarting returns a Run receipt")
    };
    let receipt = store
        .advance_run(
            lease,
            RunAdvance::InvocationStarting {
                route: InvocationRoute {
                    provider,
                    model,
                    account_id: request
                        .account_id
                        .as_ref()
                        .map(|account_id| account_id.as_str().to_string()),
                },
                surface: "headless".to_string(),
                resume_token: request.resume_token,
                answer_ask_id: None,
            },
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let AdvanceReceipt::Invocation(invocation) = receipt else {
        unreachable!("InvocationStarting returns an Invocation receipt")
    };
    let control_bin = pin_control_binary(&execution.lf_bin)
        .to_string_lossy()
        .to_string();
    let argv = vec![
        control_bin.clone(),
        "__work".to_string(),
        request.work.kind().to_string(),
        request.work.id().to_string(),
    ];
    let run_lease = lease.env_value().to_string();
    let invocation_id = invocation.id.as_str().to_string();
    let db_path = execution.db_path.to_string_lossy().to_string();
    let lf_home = execution.lf_home.to_string_lossy().to_string();
    let environment = [
        (
            crate::engine::wave_context::WAVE_ID_ENV,
            request.wave_id.as_str(),
        ),
        (crate::durable::RUN_CONTEXT_ENV, "agent"),
        (crate::durable::RUN_LEASE_ENV, run_lease.as_str()),
        (crate::durable::AGENT_INVOCATION_ENV, invocation_id.as_str()),
        (crate::store::CONTROL_BIN_ENV, control_bin.as_str()),
        (crate::store::CONTROL_DB_PATH_ENV, db_path.as_str()),
        (crate::store::CONTROL_HOME_ENV, lf_home.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, &request.cwd, &argv, &environment).await
    {
        let _ = store
            .advance_run(
                lease,
                RunAdvance::InvocationEnded {
                    invocation_id: invocation.id.clone(),
                    outcome: BoundaryState::Failed,
                },
            )
            .await;
        let _ = store
            .stop_run(
                lease,
                crate::durable::StopCause::Recovery,
                crate::durable::ContainmentObservation::Absent,
            )
            .await;
        return Err(OpsError::Message(format!(
            "failed to launch {} body: {error}",
            request.work.kind()
        )));
    }
    Ok(invocation)
}
