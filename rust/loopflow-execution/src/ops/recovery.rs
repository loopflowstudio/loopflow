//! Lazy recovery: any lf read of this Machine's Tasks or processes settles
//! AgentProcesses whose lf died, starts a shipping Task's ship process again
//! when it has exited, starts a run that ended waiting on a usage limit, and gives
//! back rented Machines whose work drained or whose soft deadline came. There is
//! no schedule and no setting; a Machine nothing reads recovers on its next read.

/// Recover this Machine's work. Failures are logged, never fatal to the read.
pub fn recover() {
    let reaped = crate::store::database_path_from_env()
        .map_err(anyhow::Error::from)
        .and_then(|database| crate::harness::agent_process::reap_agent_processes(&database));
    match reaped {
        Ok(report) => {
            for error in report.errors {
                tracing::warn!("AgentProcess settlement: {error}");
            }
        }
        Err(error) => tracing::warn!("AgentProcess settlement: {error}"),
    }
    if let Err(error) = crate::ops::task::ship::restart_ships() {
        tracing::warn!("ship restart: {error}");
    }
    if let Err(error) = crate::ops::task::limit_recovery::restart_waiting_runs() {
        tracing::warn!("limit recovery: {error}");
    }
    if let Err(error) = crate::rental::recovery::check() {
        tracing::warn!("Rentals: {error:#}");
    }
}
