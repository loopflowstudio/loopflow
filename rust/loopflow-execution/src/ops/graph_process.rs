//! A Graph process is one lf Process and the step Processes it starts. It writes
//! a GraphExecution: the Graph's name and graph as compiled at launch, then one row
//! per step it starts. The record is append-only and only the Graph process writes
//! it. Each step is an ordinary command that knows nothing of its Graph; whether
//! the Graph or a step is running, finished or failed is read from their Processes.

/// A step's agent sees which Graph started it: the Graph process's id. Steps of
/// one Graph can share notes under it. It configures nothing in lf.
pub const GRAPH_ID_ENV: &str = "LF_GRAPH_ID";

/// Whether this process is a Graph's step. A session a transport owner serves
/// starts without the Graph's environment; the step it serves is its parent.
pub fn in_graph() -> bool {
    if crate::journal::graph_step() {
        return true;
    }
    let processes: Vec<_> = [
        crate::journal::current_lf_process_id(),
        crate::journal::current_parent_lf_process_id(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if processes.is_empty() {
        return false;
    }
    let step = || -> anyhow::Result<bool> {
        let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
        for process in &processes {
            if store.graph_of_step(process)?.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    };
    step().unwrap_or_else(|error| {
        tracing::warn!(%error, "could not read whether this process is a Graph step");
        false
    })
}

/// The Task's latest interrupt when its Task run began. Each Graph attempt
/// compares later interrupts against it; steps never inherit it.
pub const TASK_INTERRUPT_CURSOR_ENV: &str = "LF_TASK_INTERRUPT_CURSOR";
