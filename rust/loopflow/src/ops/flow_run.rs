//! The driver's kernel lock and the step identity a child Exec carries in its
//! environment. Captured progression and claims live on `flow_sessions`.
use std::fs::{self, File, OpenOptions};

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::session_record::{SessionFlowMembership, SessionFlowStep};

pub(crate) const FLOW_STEP_ENV: &str = "LF_FLOW_STEP";

/// The Flow and cursor version a step's Exec was launched for. A write
/// from the step is refused once the cursor moved on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ActiveStep {
    pub invocation: String,
    pub version: u64,
}

impl ActiveStep {
    pub(crate) fn of(flow: &crate::durable::FlowSession) -> Self {
        Self {
            invocation: flow.id().to_owned(),
            version: flow.version,
        }
    }

    pub(crate) fn env_value(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

pub(crate) fn driver_lock(id: &str) -> Result<File> {
    uuid::Uuid::parse_str(id).context("invalid Flow invocation id")?;
    let dir = crate::store::lf_home_dir().join("flows").join(id);
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

/// The captured Flow position selected for this Exec and its reserved input.
/// Helpers and prepared reviews retain their independent admission paths.
#[derive(Debug)]
pub(crate) struct StepExec {
    pub membership: SessionFlowMembership,
    pub reserved: Option<(ActiveStep, i64, String)>,
}

pub(crate) fn capture_membership() -> Result<StepExec> {
    let independent = StepExec {
        membership: SessionFlowMembership::Independent,
        reserved: None,
    };
    let Some(token) = token()? else {
        return Ok(independent);
    };
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let flow = store
        .flow(&token.invocation)?
        .with_context(|| format!("Flow {} has no invocation row", token.invocation))?;
    anyhow::ensure!(
        flow.version == token.version && !flow.finished,
        "stale Flow step launch"
    );
    let reserved = match &flow.current_attempt {
        Some(attempt) if !attempt.published && flow.pending_session_id.is_none() => {
            (attempt.captured, attempt.run_id.clone())
        }
        _ => return Ok(independent),
    };
    Ok(StepExec {
        membership: SessionFlowMembership::Step(SessionFlowStep::of(&flow)?),
        reserved: Some((token, reserved.0, reserved.1)),
    })
}
