//! Stable conversations joined to exact live process evidence. SQL ownership
//! attributes observations; it never establishes liveness or control authority.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod reader;
pub(crate) use reader::ActiveSessionReader;

use crate::durable::WorkRef;
use crate::lf::commands::top::AgentProcessObservation;
use crate::store::SharedStore;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSession {
    pub id: String,
    pub title: String,
    pub work: Option<WorkRef>,
    pub processes: Vec<AgentProcessObservation>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DiscoveryState {
    Scanning,
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveSessionsSnapshot {
    pub discovery: DiscoveryState,
    pub home: PathBuf,
    pub observed_at: i64,
    pub task: Option<WorkRef>,
    pub sessions: Vec<ActiveSession>,
    pub gaps: Vec<String>,
}

pub async fn snapshot(
    home: &Path,
    store: &SharedStore,
    task: Option<WorkRef>,
) -> ActiveSessionsSnapshot {
    match ActiveSessionReader::start(home, false, tokio_util::sync::CancellationToken::new()) {
        Ok(mut reader) => reader.observe(store, task).await,
        Err(error) => ActiveSessionsSnapshot {
            home: home.to_owned(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Unavailable,
            sessions: Vec::new(),
            gaps: vec![error.to_string()],
        },
    }
}
