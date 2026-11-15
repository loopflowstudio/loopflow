//! Durable admission is independent of command execution and domain commits.
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::engine::{Error, ErrorCode};
use crate::lf::commands::subscribe::{Subscription, Transport};
use crate::lf::commands::write::Write;
use crate::store::sqlite::operations::OperationReceipt;
use crate::store::sqlite::SqliteStore;

pub fn lookup(id: &str) -> Result<Option<OperationReceipt>> {
    let store =
        crate::store::read_existing_registry()?.context("Machine registry is unavailable")?;
    Ok(store.sqlite.operation_receipt(id)?)
}

/// Both carriers observe the same receipt and operation-only revision source.
pub fn subscribe(id: String, transport: Transport) -> Result<Option<Subscription>> {
    crate::lf::commands::subscribe::start_receipts(
        id,
        |store, id: String| async move {
            let store = crate::lf::commands::subscribe::require_store(store)?;
            Ok(store.sqlite.operation_receipt(&id)?)
        },
        transport,
    )
}

pub(crate) fn admit(id: &str, command: &Write) -> Result<(SqliteStore, OperationReceipt)> {
    let span = tracing::info_span!("operation_admission", operation_id = id);
    let _entered = span.enter();
    if id.is_empty() || id.len() > 128 {
        return Err(Error::new(ErrorCode::Invalid, "operation ID must contain 1–128 bytes").into());
    }
    let payload = serde_json::to_value(command)?;
    let kind = payload["kind"]
        .as_str()
        .context("a typed command has a kind")?;
    // Creations already have a domain fingerprint and recovery contract. Keep
    // that identity when admitting their envelope, including released receipts.
    let fingerprint = match command {
        Write::TaskCreate {
            directory,
            request_id,
            title,
            request,
            wave,
            graph,
            session,
            run,
        } => {
            require_creation_id(id, request_id)?;
            let input = super::creation::TaskCreation {
                directory: crate::repo::require_repo_root(directory, "task create")?,
                title: title.clone(),
                request: request.clone(),
                wave: wave.clone(),
                graph: graph.clone(),
                session: session.clone(),
                worktree: None,
                run: *run,
                agent: None,
                approve: false,
            };
            super::creation::identity("task.create", id, &input, *run)?.fingerprint
        }
        Write::SessionNew {
            request_id,
            id: session,
            agent,
            ..
        } => {
            require_creation_id(id, request_id)?;
            super::creation::identity(
                "session.new",
                id,
                &serde_json::json!({"id":session,"agent":agent}),
                false,
            )?
            .fingerprint
        }
        _ => hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)),
    };
    let path = crate::store::database_path_from_env()?;
    anyhow::ensure!(path.exists(), "Machine registry is unavailable");
    let store = SqliteStore::new(&path)?;
    let receipt = store
        .admit_operation(id, kind, &fingerprint, &payload)
        .map_err(|error| match error {
            crate::store::StoreError::Stale(message) => anyhow::Error::from(
                Error::new(ErrorCode::Conflict, message).with(serde_json::json!({"request_id":id})),
            ),
            other => other.into(),
        })?;
    Ok((store, receipt))
}

fn require_creation_id(id: &str, creation: &str) -> Result<()> {
    if id != creation {
        return Err(Error::new(
            ErrorCode::Invalid,
            "the operation and creation must use the same request ID",
        )
        .into());
    }
    Ok(())
}

/// A claimed command never takes the generic path twice. A panic or process
/// exit leaves `dispatched`, whose effect must be established by its domain.
pub(crate) fn execute(store: &SqliteStore, id: &str, command: &Write) -> Result<()> {
    let span = tracing::info_span!("operation_dispatch", operation_id = id);
    let _entered = span.enter();
    if !store.dispatch_operation(id)? {
        return Ok(());
    }
    let output: Result<serde_json::Value> =
        crate::journal::request::with_operation(Some(id.to_owned()), || {
            crate::lf::commands::account::served(|| Ok(serde_json::to_value(command.run()?)?))
        });
    match output {
        Ok(value) => store.finish_operation(id, Ok(&value))?,
        // A command can fail after committing a stage or an external side effect.
        // Keep all commits and the error; never call this a rolled-back command.
        Err(error) => store.finish_operation(id, Err(&format!("{error:#}")))?,
    }
    Ok(())
}
