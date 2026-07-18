use std::path::PathBuf;

use crate::durable::{
    AdvanceReceipt, BoundaryState, Containment, Launch, LaunchRoute, RunAdvance, RunLease,
};
use crate::engine::process::{current_home_execution_context, start_lf_session_with_env};
use crate::id::WaveId;
use crate::store::SharedStore;

use super::{OpsError, OpsResult};

#[derive(Debug)]
pub(crate) struct RunLaunch {
    pub kind: &'static str,
    pub legacy_id: String,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub agent: String,
    pub resume_token: Option<String>,
}

pub(crate) async fn launch_in_run(
    store: &SharedStore,
    lease: &RunLease,
    request: RunLaunch,
) -> OpsResult<Launch> {
    let execution = current_home_execution_context()
        .map_err(|error| OpsError::Message(format!("cannot resolve current lf binary: {error}")))?;
    let (provider, model) = crate::engine::config::parse_agent(&request.agent);
    let tmux_name = request.tmux_name.clone();
    let receipt = store
        .advance_run(
            lease,
            RunAdvance::LaunchStarting {
                route: LaunchRoute {
                    provider,
                    model,
                    account_id: None,
                },
                containment: Containment::Tmux {
                    name: tmux_name.clone(),
                },
                cwd: request.cwd.clone(),
                surface: "headless".to_string(),
                opaque: false,
                resume_token: request.resume_token,
            },
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let AdvanceReceipt::Launch(launch) = receipt else {
        unreachable!("LaunchStarting returns a Launch receipt")
    };
    let argv = vec![
        execution.lf_bin.to_string_lossy().to_string(),
        format!("__{}", request.kind),
        request.legacy_id.clone(),
    ];
    let run_lease = lease.env_value().to_string();
    let control_bin = execution.lf_bin.to_string_lossy().to_string();
    let db_path = execution.db_path.to_string_lossy().to_string();
    let lf_home = execution.lf_home.to_string_lossy().to_string();
    let legacy_env = match request.kind {
        "project" => "LF_PROJECT_SESSION_ID",
        "task" => "LF_TASK_SESSION_ID",
        kind => return Err(OpsError::Message(format!("unsupported Run body {kind}"))),
    };
    let environment = [
        (
            crate::engine::wave_context::WAVE_ID_ENV,
            request.wave_id.as_str(),
        ),
        (legacy_env, request.legacy_id.as_str()),
        (crate::durable::RUN_CONTEXT_ENV, "agent"),
        (crate::durable::RUN_LEASE_ENV, run_lease.as_str()),
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
                RunAdvance::LaunchEnded {
                    launch_id: launch.id.clone(),
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
            request.kind
        )));
    }
    Ok(launch)
}
