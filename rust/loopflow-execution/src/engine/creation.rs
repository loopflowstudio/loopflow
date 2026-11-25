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
    // Top-level retries keep the fingerprint recorded before Tasks could have parents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
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
    tracing::info_span!(target: crate::journal::START_TIMING_TARGET, "creation_step", step = name)
        .in_scope(|| {
            let started = Instant::now();
            let result = run();
            tracing::info!(
                target: crate::journal::START_TIMING_TARGET,
                elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
                "Creation step finished"
            );
            result
        })
}

/// A created Task's receipt and, for a conversation-admitting `run`, its
/// primary conversation's record read when answering, and the planning
/// exchange its caller runs after answering. The application adds the Task's
/// row to make `task.create`'s answer.
#[derive(Debug)]
pub struct Created {
    pub receipt: TaskCreateReceipt,
    pub session: Option<crate::ops::human_session::SessionRecord>,
    pub after: Option<After>,
}

fn answered(store: &crate::store::SharedStore, receipt: TaskCreateReceipt) -> Result<Created> {
    let session = step("read_session", || {
        receipt
            .session_id
            .as_ref()
            .map(|id| {
                crate::runtime::block_on(crate::ops::human_session::primary::admitted_record(
                    store,
                    id.as_str(),
                ))
            })
            .transpose()
    })?;
    Ok(Created {
        receipt,
        session,
        after: None,
    })
}

/// Planning exchange and the assignee check following an answer, under the
/// creation request's attribution. A served answer hands it to the server,
/// which starts it once the answer is written; a CLI caller runs it after
/// printing.
pub struct After(Box<dyn FnOnce() + Send>);

impl After {
    pub fn run(self) {
        (self.0)()
    }
}

impl std::fmt::Debug for After {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("After")
    }
}

pub fn create(id: &str, mut input: TaskCreation) -> Result<Created> {
    let _creation = tracing::info_span!(target: crate::journal::START_TIMING_TARGET, "task_creation", request_id = id).entered();
    // CLI and served callers may spell the same checkout through different paths.
    input.directory = step("resolve_repository", || {
        crate::repo::require_repo_root(&input.directory, "task create")
    })?;
    let identity = identity("task.create", id, &input, input.run)?;
    let _lock = step("creation_lock", || lock(id))?;
    let store = step("open_store", || -> Result<_> {
        Ok(Arc::new(crate::runtime::block_on(open_store(
            &storage_config_from_env()?,
        ))?))
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
            let parent = input.parent.clone();
            step("file_task", || {
                if admits {
                    crate::ops::task::admit_task(
                        &input.directory,
                        input.wave.as_deref(),
                        input.title.clone(),
                        input.request.clone(),
                        adoption,
                        parent,
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
                        parent,
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
            return start_conversation(id, &input, &store, receipt)
        }
        RecoveryDisposition::Complete => return answered(&store, receipt),
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
        store
            .sqlite
            .advance_creation(id, &receipt.milestone, Some(&receipt.stored_result()?))?;
        Ok(())
    })?;
    answered(&store, receipt)
}

/// Start the admitted conversation held and answer. The conversation makes
/// the checkout and starts the Graph; planning exchange follows the answer.
fn start_conversation(
    id: &str,
    input: &TaskCreation,
    store: &crate::store::SharedStore,
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
        match crate::runtime::block_on(crate::ops::human_session::primary::hold_task_conversation(
            store,
            &task,
            &session,
            input.agent.as_deref(),
            id,
        )) {
            Ok(true) => {}
            Ok(false) => tracing::info!("new Task conversation waits for its first open"),
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "new Task conversation did not start")
            }
        }
    });
    let mut created = answered(store, receipt)?;
    let main = crate::git::worktrees::main_repo_root(&input.directory)?;
    let span = tracing::Span::current();
    let sync = move || {
        let _creation = span.enter();
        step("sync_planning", || {
            crate::ops::task::sync_admitted(&main, &task.id)
        })
    };
    created.after = crate::journal::request::after_reply(sync).map(|sync| After(Box::new(sync)));
    Ok(created)
}

pub fn unknown(receipt: &CreationReceipt) -> Error {
    Error::new(ErrorCode::OutcomeUnknown, &receipt.explanation)
        .with(serde_json::json!({"request_id":receipt.request_id,"milestone":receipt.milestone,"task_id":receipt.task_id,"session_id":receipt.session_id}))
}
