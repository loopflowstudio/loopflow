//! Lazy recovery: any lf read of this Machine's Tasks or processes settles
//! AgentProcesses whose lf died and starts a shipping Task's ship process again
//! when it has exited. There is no schedule and no setting; a Machine nothing
//! reads recovers on its next read.

/// Recover this Machine's work. Failures are logged, never fatal to the read.
pub fn recover() {
    if let Err(error) = crate::harness::agent_process::reap_agent_processes(false) {
        tracing::warn!("AgentProcess settlement: {error}");
    }
    if let Err(error) = crate::ops::task::ship::restart_ships() {
        tracing::warn!("ship restart: {error}");
    }
}
