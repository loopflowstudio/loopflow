//! Task creation's receipt is shared by CLI and served callers.
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Error, ErrorCode};
use crate::store::{open_store, storage_config_from_env};
use loopflow_storage::sqlite::creation::{CreationIdentity, CreationReceipt, RecoveryDisposition};

pub use loopflow_storage::sqlite::creation::TaskCreateReceipt;

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

pub fn identity(
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

pub fn lock(id: &str) -> Result<File> {
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

/// Bounded creation stages are visible under the served request's trace. Never
/// record the request text, title, generated prompt or provider output.
pub fn step<T>(name: &'static str, run: impl FnOnce() -> T) -> T {
    tracing::info_span!("creation_step", step = name).in_scope(|| {
        let started = Instant::now();
        let result = run();
        tracing::info!(
            elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
            "Creation step finished"
        );
        result
    })
}

/// A created Task's receipt, the primary conversation's record and the Task's
/// overview, read when answering. The application adds the Task's row to make
/// `task.create`'s answer.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskCreated {
    #[serde(flatten)]
    pub receipt: TaskCreateReceipt,
    #[serde(deserialize_with = "Option::deserialize")]
    pub session: Option<crate::ops::human_session::SessionRecord>,
    pub overview: crate::task_work::TaskOverview,
}

/// A created Task, and for a conversation-admitting `run` the planning
/// exchange still running after the answer.
#[derive(Debug)]
pub struct Created {
    pub answer: TaskCreated,
    pub after: Option<After>,
}

fn answer(
    store: &crate::store::SharedStore,
    runtime: &tokio::runtime::Runtime,
    receipt: TaskCreateReceipt,
) -> Result<TaskCreated> {
    step("read_answer", || {
        let session = receipt
            .session_id
            .as_ref()
            .map(|id| {
                runtime.block_on(crate::ops::human_session::primary::admitted_record(
                    store,
                    id.as_str(),
                ))
            })
            .transpose()?;
        let (overview, _) =
            crate::store::task_work::task_overview(&store.sqlite, &receipt.task.id)?;
        Ok(TaskCreated {
            receipt,
            session,
            overview,
        })
    })
}

/// Planning exchange and the assignee check following an answer, under the
/// creation request's attribution. A caller that exits joins it; dropping it
/// lets it finish on its own, as the server does.
#[derive(Debug)]
pub struct After {
    runtime: Option<tokio::runtime::Runtime>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl After {
    pub fn join(mut self) {
        let (Some(runtime), Some(handle)) = (self.runtime.take(), self.handle.take()) else {
            return;
        };
        if runtime.block_on(handle).is_err() {
            tracing::warn!("planning sync after creation panicked");
        }
    }
}

impl Drop for After {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

pub fn create(id: &str, mut input: TaskCreation) -> Result<Created> {
    let _creation = tracing::info_span!("task_creation", request_id = id).entered();
    // CLI and served callers may spell the same checkout through different paths.
    input.directory = step("resolve_repository", || {
        crate::repo::require_repo_root(&input.directory, "task create")
    })?;
    let identity = identity("task.create", id, &input, input.run)?;
    let _lock = step("creation_lock", || lock(id))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let store = step("open_store", || -> Result<_> {
        Ok(Arc::new(
            runtime.block_on(open_store(&storage_config_from_env()?))?,
        ))
    })?;
    step("check_receipt", || store.sqlite.check_creation(&identity)).map_err(
        |error| match error {
            crate::store::StoreError::Stale(message) => anyhow::Error::from(
                Error::new(ErrorCode::Conflict, message).with(serde_json::json!({"request_id":id})),
            ),
            other => other.into(),
        },
    )?;
    // A started Task with a conversation commits Task, Graph, claim and
    // conversation together and answers before any network or checkout.
    let admits = input.run && !input.approve;
    let (recorded, filed) = match store.sqlite.creation_receipt(id)? {
        Some(receipt) => (receipt, false),
        None => {
            if let Some(graph) = input.graph.as_deref() {
                crate::ops::task::load_graph(graph, "", &input.directory)?;
            }
            let adoption = crate::ops::task::TaskAdoptionOptions {
                session: input.session.clone(),
                worktree: input.worktree.clone(),
            };
            step("file_task", || {
                if admits {
                    crate::ops::task::admit_task(
                        &input.directory,
                        input.wave.as_deref(),
                        input.title.clone(),
                        input.request.clone(),
                        adoption,
                        input.graph.as_deref(),
                        identity,
                    )
                } else {
                    crate::ops::task::task_create(
                        &input.directory,
                        input.wave.as_deref(),
                        input.title.clone(),
                        input.request.clone(),
                        adoption,
                        Some(identity),
                    )
                }
            })?;
            let receipt = store
                .sqlite
                .creation_receipt(id)?
                .context("creation receipt is missing")?;
            (receipt, true)
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
        RecoveryDisposition::Complete if filed && admits => {
            return start_conversation(&input, &store, runtime, receipt)
        }
        RecoveryDisposition::Complete => {
            return Ok(Created {
                answer: answer(&store, &runtime, receipt)?,
                after: None,
            })
        }
        RecoveryDisposition::Continue if !admits => {}
        _ => return Err(unknown(&recorded).into()),
    }
    // `-b` runs without a conversation; its carrier reports the launch.
    if recorded.milestone == "filed" {
        store.sqlite.advance_creation(id, "run_requested", None)?;
        let mut args = vec!["--batch".to_string()];
        if let Some(agent) = &input.agent {
            args.extend(["--agent".into(), agent.clone()]);
        }
        args.extend(["task".into(), "run".into(), receipt.task.identifier.clone()]);
        args.extend(input.graph.iter().cloned());
        args.push("--json".into());
        let launched: super::task::TaskRun = step("launch_graph", || {
            super::launch::start_creation(&input.directory, &args, id)
        })?;
        store
            .sqlite
            .record_creation_launch(id, &serde_json::to_value(launched)?)?;
    }
    let graph = crate::store::task_work::task_graph(&store.sqlite, &receipt.task.id)?
        .context("the run did not take up a Graph")?
        .name;
    receipt.graph = Some(graph);
    receipt.milestone = "started".into();
    step("admit_receipt", || -> Result<()> {
        store.sqlite.advance_creation(
            id,
            &receipt.milestone,
            Some(&serde_json::to_value(&receipt)?),
        )?;
        Ok(())
    })?;
    Ok(Created {
        answer: answer(&store, &runtime, receipt)?,
        after: None,
    })
}

/// Start the admitted conversation held and answer. The conversation makes
/// the checkout and starts the Graph; planning exchange follows the answer.
fn start_conversation(
    input: &TaskCreation,
    store: &crate::store::SharedStore,
    runtime: tokio::runtime::Runtime,
    receipt: TaskCreateReceipt,
) -> Result<Created> {
    let task = receipt.task.clone();
    let session = receipt
        .session_id
        .as_ref()
        .context("admitted creation has no conversation")?;
    let session = store
        .sqlite
        .session(session.as_str())?
        .context("admitted conversation is missing")?;
    // A conversation that does not start here starts, and sets up, when opened.
    step("hold_conversation", || {
        match runtime.block_on(crate::ops::human_session::primary::hold_task_conversation(
            store,
            &task,
            &session,
            input.agent.as_deref(),
        )) {
            Ok(true) => {}
            Ok(false) => tracing::info!("new Task conversation waits for its first open"),
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "new Task conversation did not start")
            }
        }
    });
    let answer = answer(store, &runtime, receipt)?;
    let main = crate::git::worktrees::main_repo_root(&input.directory)?;
    let handle = {
        let _runtime = runtime.enter();
        crate::journal::request::spawn_blocking(move || {
            step("sync_planning", || {
                crate::ops::task::sync_admitted(&main, &task.id)
            })
        })
    };
    Ok(Created {
        answer,
        after: Some(After {
            runtime: Some(runtime),
            handle: Some(handle),
        }),
    })
}

pub fn unknown(receipt: &CreationReceipt) -> Error {
    Error::new(ErrorCode::OutcomeUnknown, &receipt.explanation)
        .with(serde_json::json!({"request_id":receipt.request_id,"milestone":receipt.milestone,"task_id":receipt.task_id,"session_id":receipt.session_id}))
}
