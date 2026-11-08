//! Task creation's receipt is shared by CLI and served callers.
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Error, ErrorCode};
use crate::id::LfSessionId;
use crate::store::sqlite::creation::{CreationIdentity, CreationReceipt, RecoveryDisposition};
use crate::store::{open_store, storage_config_from_env};
use crate::work::task::Task;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskCreateReceipt {
    pub task: Task,
    pub graph: Option<String>,
    pub session_id: Option<LfSessionId>,
    pub milestone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreation {
    pub directory: PathBuf,
    pub title: Option<String>,
    pub request: Option<String>,
    pub wave: Option<String>,
    pub graph: Option<String>,
    pub session: Option<String>,
    pub worktree: Option<PathBuf>,
    pub run: bool,
    pub agent: Option<String>,
    pub approve: bool,
}

pub(crate) fn identity(
    kind: &'static str,
    id: &str,
    payload: &impl Serialize,
    run: bool,
) -> Result<CreationIdentity> {
    if id.is_empty() || id.len() > 128 {
        return Err(Error::new(
            ErrorCode::Invalid,
            "creation request ID must contain 1–128 bytes",
        )
        .into());
    }
    // Value's sorted map keys make struct field order immaterial.
    let canonical = serde_json::to_vec(&serde_json::to_value(payload)?)?;
    Ok(CreationIdentity {
        request_id: id.into(),
        kind,
        run,
        fingerprint: hex::encode(Sha256::digest(canonical)),
    })
}

pub(crate) fn lock(id: &str) -> Result<File> {
    let root = crate::store::lf_home_dir().join("creation-locks");
    std::fs::create_dir_all(&root)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(hex::encode(Sha256::digest(id.as_bytes()))))?;
    file.lock_exclusive()?;
    Ok(file)
}

pub fn lookup(id: &str) -> Result<Option<CreationReceipt>> {
    // Failure to open the registry is never confirmed absence.
    let store =
        crate::store::read_existing_registry()?.context("Machine registry is unavailable")?;
    Ok(store.sqlite.creation_receipt(id)?)
}

pub fn create(id: &str, input: TaskCreation) -> Result<TaskCreateReceipt> {
    let identity = identity("task.create", id, &input, input.run)?;
    let _lock = lock(id)?;
    let store = Arc::new(
        tokio::runtime::Runtime::new()?.block_on(open_store(&storage_config_from_env()?))?,
    );
    store
        .sqlite
        .check_creation(&identity)
        .map_err(|error| match error {
            crate::store::StoreError::Stale(message) => anyhow::Error::from(
                Error::new(ErrorCode::Conflict, message).with(serde_json::json!({"request_id":id})),
            ),
            other => other.into(),
        })?;
    let recorded = match store.sqlite.creation_receipt(id)? {
        Some(receipt) => receipt,
        None => {
            if let Some(graph) = input.graph.as_deref() {
                crate::ops::task::load_graph(graph, "", &input.directory)?;
            }
            crate::ops::task::task_create(
                &input.directory,
                input.wave.as_deref(),
                input.title.clone(),
                input.request.clone(),
                crate::ops::task::TaskAdoptionOptions {
                    session: input.session.clone(),
                    worktree: input.worktree.clone(),
                },
                Some(identity),
            )?;
            store
                .sqlite
                .creation_receipt(id)?
                .context("creation receipt is missing")?
        }
    };
    let mut receipt: TaskCreateReceipt = serde_json::from_value(
        recorded
            .result
            .clone()
            .context("filed receipt has no result")?,
    )?;
    match recorded.disposition {
        RecoveryDisposition::Inspect => return Err(unknown(&recorded).into()),
        RecoveryDisposition::Complete => return Ok(receipt),
        RecoveryDisposition::Continue => {}
    }
    // Save the first turn before task run can start the conversation beside its Graph.
    let session_id = if input.approve {
        None
    } else {
        Some(
            tokio::runtime::Runtime::new()?
                .block_on(crate::ops::human_session::primary::ensure_task(
                    &store,
                    &input.directory,
                    &receipt.task.identifier,
                    input.request.clone(),
                ))?
                .0
                .id,
        )
    };
    // The projection permits either the first launch or finishing its result.
    if recorded.milestone == "filed" {
        store.sqlite.advance_creation(id, "run_requested", None)?;
        let mut args = Vec::new();
        if input.approve {
            args.push("--batch".into());
        }
        if let Some(agent) = &input.agent {
            args.extend(["--agent".into(), agent.clone()]);
        }
        args.extend(["task".into(), "run".into(), receipt.task.identifier.clone()]);
        args.extend(input.graph.iter().cloned());
        args.push("--json".into());
        let launched: super::task::TaskRun =
            super::launch::start_creation(&input.directory, &args, id)?;
        store
            .sqlite
            .record_creation_launch(id, &serde_json::to_value(launched)?)?;
    }
    let graph = store
        .sqlite
        .task_graph(&receipt.task.id)?
        .context("the run did not take up a Graph")?
        .name;
    receipt.graph = Some(graph);
    receipt.session_id = session_id;
    receipt.milestone = if input.approve { "started" } else { "admitted" }.into();
    store.sqlite.advance_creation(
        id,
        &receipt.milestone,
        Some(&serde_json::to_value(&receipt)?),
    )?;
    Ok(receipt)
}

pub(crate) fn unknown(receipt: &CreationReceipt) -> Error {
    Error::new(ErrorCode::OutcomeUnknown, &receipt.explanation)
        .with(serde_json::json!({"request_id":receipt.request_id,"milestone":receipt.milestone,"task_id":receipt.task_id,"session_id":receipt.session_id}))
}
