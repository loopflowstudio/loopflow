//! What a saved Flow keeps outside its invocation row: the driver's kernel
//! lock and the step identity a Run carries in its environment. The cursor,
//! attempt and failure live on `flow_invocations`.
use std::fs::{self, File, OpenOptions};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

pub(crate) const FLOW_STEP_ENV: &str = "LF_FLOW_STEP";

/// The invocation and cursor version a step's Run was launched for. A write
/// from the Run is refused once the cursor moved on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ActiveStep {
    pub invocation: String,
    pub version: u64,
}

pub(crate) fn driver_lock(id: &str) -> Result<File> {
    uuid::Uuid::parse_str(id).context("invalid Flow invocation id")?;
    let dir = crate::store::current_home_lf_home_dir()
        .join("flows")
        .join(id);
    fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("driver.lock"))?;
    FileExt::lock_exclusive(&file)?;
    Ok(file)
}

pub(crate) fn token() -> Result<Option<ActiveStep>> {
    std::env::var(FLOW_STEP_ENV)
        .ok()
        .map(|s| serde_json::from_str(&s).context("invalid active Flow step identity"))
        .transpose()
}

/// The Flow step a launch executes, from the row. A review's Run is prepared
/// with its Session, and a launch inside a step that already has its Run is
/// a helper: both are independent.
pub(crate) fn capture_membership() -> Result<crate::run_record::RunFlowMembership> {
    use crate::run_record::{RunFlowMembership, RunFlowStep};
    let Some(token) = token()? else {
        return Ok(RunFlowMembership::Independent);
    };
    let store =
        crate::store::sqlite::SqliteStore::new(&crate::store::observability_database_path()?)?;
    let flow = store
        .flow(&token.invocation)?
        .with_context(|| format!("Flow {} has no invocation row", token.invocation))?;
    anyhow::ensure!(
        flow.version == token.version && !flow.finished,
        "stale Flow step launch"
    );
    if flow.pending_session_id.is_some() || flow.current_attempt.is_some() || flow.is_human() {
        return Ok(RunFlowMembership::Independent);
    }
    Ok(RunFlowMembership::Step(RunFlowStep::of_flow(&flow)?))
}

/// Request the existing Home process supervisor to continue this saved invocation.
#[cfg(not(test))]
pub(crate) async fn launch_driver(id: &str) -> Result<()> {
    let store = crate::store::open_store(&crate::store::storage_config_from_env()?).await?;
    let flow = store
        .flow(id)
        .await?
        .with_context(|| format!("Flow {id} has no invocation row"))?;
    let lf = crate::engine::process::resolve_current_home_lf_binary_checked()?;
    let argv = vec![
        lf.display().to_string(),
        "-b".into(),
        "flow".into(),
        "resume".into(),
        id.into(),
    ];
    crate::engine::process::start_home_session(&format!("lf-flow-{id}"), &flow.cwd, &argv).await
}

#[cfg(test)]
pub(crate) async fn launch_driver(_id: &str) -> Result<()> {
    Ok(())
}
