//! Diagnosis shared by process inspection and application reports.

use crate::process::ProcessEntry;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

impl Check {
    pub fn ok(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Ok,
            detail: detail.into(),
        }
    }
    pub fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Warn,
            detail: detail.into(),
        }
    }
    pub fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Fail,
            detail: detail.into(),
        }
    }
}

pub fn check_lineage(events: &[ProcessEntry], repairable: usize) -> Check {
    let processes: HashMap<&str, &str> = events
        .iter()
        .map(|event| (event.id(), event.trace_id().as_str()))
        .collect();
    let dangling: HashSet<&str> = events
        .iter()
        .filter_map(|event| {
            let parent = event.parent_lf_process_id()?.as_str();
            (processes.get(parent).copied() != Some(event.trace_id().as_str())).then_some(parent)
        })
        .collect();
    if dangling.is_empty() {
        return Check::ok("lineage", "every parent process resolves");
    }
    let mut detail = format!(
        "{} parent process id(s) are missing or belong to another trace",
        dangling.len()
    );
    if repairable > 0 {
        detail.push_str(&format!(
            "; {repairable} AgentProcess parent(s) were never recorded, and the server's recovery pass repairs them"
        ));
    }
    Check::fail("lineage", detail)
}
