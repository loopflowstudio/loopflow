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
use crate::store::sqlite::creation::{CreationIdentity, CreationReceipt};
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
    let identity = identity("task.create", id, &input)?;
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
    let saved = store.sqlite.creation_receipt(id)?;
    if let Some(graph) = input.graph.as_deref().filter(|_| saved.is_none()) {
        crate::ops::task::load_graph(graph, "", &input.directory)?;
    }
    let mut receipt: TaskCreateReceipt = match saved.as_ref().and_then(|r| r.result.as_ref()) {
        Some(result) => serde_json::from_value(result.clone())?,
        None => {
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
            serde_json::from_value(
                store
                    .sqlite
                    .creation_receipt(id)?
                    .context("creation receipt is missing")?
                    .result
                    .context("filed receipt has no result")?,
            )?
        }
    };
    // A receipt never restores a removed resource.
    if store.sqlite.task(&receipt.task.id)?.is_none() {
        return Err(Error::new(
            ErrorCode::NotFound,
            "the Task recorded by this creation request was removed",
        )
        .with(serde_json::json!({"request_id": id, "task_id": receipt.task.id}))
        .into());
    }
    if !input.run || receipt.milestone == "admitted" {
        return Ok(receipt);
    }
    let recorded = store
        .sqlite
        .creation_receipt(id)?
        .context("creation receipt is missing")?;
    match recorded.milestone.as_str() {
        "filed" => {
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
        "run_requested" => return Err(unknown(&recorded).into()),
        "started" | "admitted" => {}
        _ => anyhow::bail!("invalid Task creation milestone {}", recorded.milestone),
    }
    let graph = store
        .sqlite
        .task_graph(&receipt.task.id)?
        .context("the run did not take up a Graph")?
        .name;
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
    Error::new(ErrorCode::OutcomeUnknown, "creation's external effect is unconfirmed; inspect the recorded resource")
        .with(serde_json::json!({"request_id":receipt.request_id,"milestone":receipt.milestone,"task_id":receipt.task_id,"session_id":receipt.session_id}))
}
