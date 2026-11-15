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

pub(crate) fn admit(
    id: &str,
    epoch: &str,
    command: &Write,
) -> Result<(SqliteStore, OperationReceipt)> {
    let span = tracing::info_span!("operation_admission", operation_id = id);
    let _entered = span.enter();
    if id.is_empty() || id.len() > 128 {
        return Err(Error::new(ErrorCode::Invalid, "operation ID must contain 1–128 bytes").into());
    }
    let payload = serde_json::to_value(command)?;
    let kind = payload["kind"]
        .as_str()
        .context("a typed command has a kind")?;
    match command {
        Write::TaskCreate { request_id, .. } | Write::SessionNew { request_id, .. } => {
            require_creation_id(id, request_id)?
        }
        _ => {}
    }
    // Typed values and sorted object keys give submitted content a stable v1
    // identity. Repository resolution belongs to validation, after admission.
    let mut content = payload.clone();
    content
        .as_object_mut()
        .context("a typed command is an object")?
        .remove("request_id");
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(
        &serde_json::json!({"version":1,"command":content}),
    )?));
    let path = crate::store::database_path_from_env()?;
    anyhow::ensure!(path.exists(), "Machine registry is unavailable");
    let store = SqliteStore::new(&path)?;
    let receipt = store
        .admit_operation(epoch, id, kind, &fingerprint, &payload)
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
pub(crate) fn execute(store: &SqliteStore, id: &str) -> Result<()> {
    let span = tracing::info_span!("operation_dispatch", operation_id = id);
    let _entered = span.enter();
    let Some(saved) = store.submitted_command(id)? else {
        return Ok(());
    };
    let command: Write = serde_json::from_value(saved.command)?;
    if matches!(
        saved.validation,
        crate::store::sqlite::operations::OperationValidation::Pending
            | crate::store::sqlite::operations::OperationValidation::Candidate
    ) {
        let identity = match creation_identity(id, &command) {
            Ok(identity) => identity,
            Err(error) => {
                // Unavailable repository discovery is retryable validation, never
                // a content mismatch and never permission to replace the envelope.
                store.record_validation_error(id, saved.generation, &format!("{error:#}"))?;
                return Ok(());
            }
        };
        if !store.validate_operation(id, saved.generation, &identity)? {
            return Ok(());
        }
    }
    if !store.dispatch_operation(id)? {
        // Only the existing domain receipt can authorize continuation after a
        // dispatch. Generic commands and creations without domain facts stay held.
        if !matches!(
            saved.delivery,
            crate::store::sqlite::operations::OperationDelivery::Dispatched
                | crate::store::sqlite::operations::OperationDelivery::Uncertain
        ) {
            return Ok(());
        }
        match &command {
            Write::TaskComplete { .. } => return recover_completion(store, id),
            Write::TaskReopen { .. } => {
                store.recover_operation_result(id, "task.reopened")?;
                return Ok(());
            }
            _ => {}
        }
        let Some(creation) = store.creation_receipt(id)? else {
            return Ok(());
        };
        if creation.disposition == crate::store::sqlite::operations::RecoveryDisposition::Inspect {
            return Ok(());
        }
    }
    let output: Result<serde_json::Value> =
        crate::journal::request::with_operation(Some(id.to_owned()), || {
            crate::lf::commands::account::served(|| Ok(serde_json::to_value(command.run()?)?))
        });
    match output {
        Ok(value) => store.finish_operation(id, Ok(&value))?,
        Err(error) => store.finish_operation(id, Err(&format!("{error:#}")))?,
    }
    Ok(())
}

fn recover_completion(store: &SqliteStore, id: &str) -> Result<()> {
    if store.recover_operation_result(id, "task.completed")? {
        return Ok(());
    }
    let Some(receipt) = store.operation_receipt(id)? else {
        return Ok(());
    };
    if receipt.delivery == crate::store::sqlite::operations::OperationDelivery::Finished {
        return Ok(());
    }
    let Some(commit) = receipt
        .commits
        .iter()
        .find(|commit| commit.stage == "task.completion_requested")
    else {
        return Ok(()); // No domain request: never call task_complete again.
    };
    let task = commit
        .resources
        .first()
        .context("completion request has a Task")?;
    let request = commit.result["request"]
        .as_i64()
        .context("completion has a saved request")?;
    let result: Result<()> = crate::journal::request::with_operation(Some(id.to_owned()), || {
        crate::lf::commands::account::served(|| {
            Ok(crate::ops::task::recover_task_completion(
                &crate::durable::TaskId::parse(task)?,
                request,
            )?)
        })
    });
    if let Err(error) = result {
        store.finish_operation(id, Err(&format!("{error:#}")))?;
    }
    // Even a successful continuation must establish its own settlement stage.
    // A newer completion cannot supply this operation's result.
    store.recover_operation_result(id, "task.completed")?;
    Ok(())
}

fn creation_identity(
    id: &str,
    command: &Write,
) -> Result<crate::store::sqlite::operations::CreationIdentity> {
    match command {
        Write::TaskCreate {
            directory,
            title,
            request,
            wave,
            graph,
            session,
            run,
            ..
        } => {
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
            super::creation::identity("task.create", id, &input, *run)
        }
        Write::SessionNew {
            id: session, agent, ..
        } => super::creation::identity(
            "session.new",
            id,
            &serde_json::json!({"id":session,"agent":agent}),
            false,
        ),
        _ => anyhow::bail!("only creations require domain identity validation"),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use crate::lf::commands::graph::EnvVarGuard;
    use crate::lf::commands::write::Write;
    use crate::store::sqlite::operations::{OperationDelivery, OperationValidation};
    use crate::store::sqlite::SqliteStore;

    fn command(directory: std::path::PathBuf, id: &str) -> Write {
        Write::TaskCreate {
            directory,
            request_id: id.into(),
            title: Some("Submitted title".into()),
            request: None,
            wave: None,
            graph: None,
            session: None,
            run: false,
        }
    }

    #[test]
    fn completion_recovery_never_reissues_a_withdrawn_request_or_replaces_its_committed_result() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let store = SqliteStore::new(&crate::store::default_db_path()).unwrap();
        let repository_id = store
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: crate::durable::TaskId::new(),
                repository_id,
                project_id: None,
                title: "Recovery".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let complete = Write::TaskComplete {
            directory: repo.path().to_owned(),
            issue: task.identifier.clone(),
        };
        let epoch = store.reading_identity().unwrap().1;
        super::admit("without-request", &epoch, &complete).unwrap();
        assert!(store.dispatch_operation("without-request").unwrap());
        super::execute(&store, "without-request").unwrap();
        assert!(store.task_completion_pending(&task.id).unwrap().is_none());
        assert_eq!(
            store
                .operation_receipt("without-request")
                .unwrap()
                .unwrap()
                .delivery,
            OperationDelivery::Dispatched
        );
        super::admit("withdrawn", &epoch, &complete).unwrap();
        assert!(store.dispatch_operation("withdrawn").unwrap());
        crate::journal::request::with_operation(Some("withdrawn".into()), || {
            store.request_task_completion(&task.id, None).unwrap();
        });
        store.reopen_task(&task.id, None).unwrap();
        super::execute(&store, "withdrawn").unwrap();
        assert!(store.task_completion_pending(&task.id).unwrap().is_none());
        let withdrawn = store.operation_receipt("withdrawn").unwrap().unwrap();
        assert_eq!(withdrawn.delivery, OperationDelivery::Uncertain);
        assert_eq!(withdrawn.commits.len(), 1);

        super::admit("settled", &epoch, &complete).unwrap();
        assert!(store.dispatch_operation("settled").unwrap());
        let request = crate::journal::request::with_operation(Some("settled".into()), || {
            store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap()
        });
        store.complete_task(&task, request).unwrap();
        let original = store.operation_receipt("settled").unwrap().unwrap().commits[1].clone();
        store
            .finish_operation("settled", Err("reply lost"))
            .unwrap();
        store.reopen_task(&task.id, None).unwrap();
        drop(store);
        let store = SqliteStore::new(&crate::store::default_db_path()).unwrap();
        super::execute(&store, "settled").unwrap();
        let recovered = store.operation_receipt("settled").unwrap().unwrap();
        assert_eq!(recovered.delivery, OperationDelivery::Finished);
        assert_eq!(recovered.result, Some(original.result));
        assert_eq!(recovered.error.as_deref(), Some("reply lost"));
        assert_ne!(
            store.task(&task.id).unwrap().unwrap().state.as_deref(),
            Some("completed")
        );
        let revision = recovered.revision;
        super::execute(&store, "settled").unwrap();
        assert_eq!(
            store
                .operation_receipt("settled")
                .unwrap()
                .unwrap()
                .revision,
            revision
        );
        super::admit("continuing", &epoch, &complete).unwrap();
        assert!(store.dispatch_operation("continuing").unwrap());
        let request = crate::journal::request::with_operation(Some("continuing".into()), || {
            store
                .request_task_completion(&task.id, None)
                .unwrap()
                .unwrap()
        });
        drop(store);
        let store = SqliteStore::new(&crate::store::default_db_path()).unwrap();
        super::execute(&store, "continuing").unwrap();
        let continued = store.operation_receipt("continuing").unwrap().unwrap();
        assert_eq!(continued.delivery, OperationDelivery::Finished);
        assert_eq!(continued.commits.len(), 2);
        assert_eq!(continued.commits[0].result["request"], request);
        assert_eq!(continued.commits[1].stage, "task.completed");
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().state.as_deref(),
            Some("completed")
        );
        let reopen = Write::TaskReopen {
            directory: repo.path().to_owned(),
            issue: task.identifier.clone(),
        };
        super::admit("reopened", &epoch, &reopen).unwrap();
        assert!(store.dispatch_operation("reopened").unwrap());
        let original = crate::journal::request::with_operation(Some("reopened".into()), || {
            store.reopen_task(&task.id, None).unwrap()
        });
        let request = store
            .request_task_completion(&task.id, None)
            .unwrap()
            .unwrap();
        store.complete_task(&task, request).unwrap();
        super::execute(&store, "reopened").unwrap();
        assert_eq!(
            store.operation_receipt("reopened").unwrap().unwrap().result,
            Some(serde_json::to_value(original).unwrap())
        );
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().state.as_deref(),
            Some("completed")
        );
    }

    #[test]
    fn admission_saves_exact_content_before_git_and_unavailable_validation_is_recoverable() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let path = crate::store::default_db_path();
        let store = SqliteStore::new(&path).unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let marker = home.path().join("git-invoked");
        let git = bin.join("git");
        std::fs::write(
            &git,
            format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _path = EnvVarGuard::set("PATH", bin.to_str().unwrap());
        let epoch = store.reading_identity().unwrap().1;
        let submitted = command(home.path().to_path_buf(), "new");
        let (_, receipt) = super::admit("new", &epoch, &submitted).unwrap();
        assert!(!marker.exists(), "admission must not invoke Git");
        assert_eq!(receipt.validation, OperationValidation::Pending);
        assert_eq!(receipt.fingerprint, None);
        assert!(store.creation_receipt("new").unwrap().is_none());
        super::execute(&store, "new").unwrap();
        assert!(marker.exists());
        let pending = store.operation_receipt("new").unwrap().unwrap();
        assert_eq!(pending.delivery, OperationDelivery::Queued);
        assert_eq!(pending.validation, OperationValidation::Pending);
        assert!(pending.error.is_some());
        assert!(pending.commits.is_empty());
        drop(store);
        let (reopened, duplicate) = super::admit("new", &epoch, &submitted).unwrap();
        assert_eq!(duplicate.revision, pending.revision);
        assert_eq!(
            reopened.submitted_command("new").unwrap().unwrap().command,
            serde_json::to_value(submitted).unwrap()
        );
        // IDs are excluded from content identity, but not from saved commands.
        let (_, another) = super::admit(
            "other",
            &epoch,
            &command(home.path().to_path_buf(), "other"),
        )
        .unwrap();
        assert_eq!(another.submitted_fingerprint, receipt.submitted_fingerprint);
    }

    #[test]
    fn adopting_a_completed_cli_creation_returns_the_same_task_without_another_effect() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let subdirectory = repo.path().join("nested");
        std::fs::create_dir(&subdirectory).unwrap();
        let original = crate::engine::creation::create(
            "cli",
            crate::engine::creation::TaskCreation {
                directory: repo.path().to_path_buf(),
                title: Some("Submitted title".into()),
                request: None,
                wave: None,
                graph: None,
                session: None,
                worktree: None,
                run: false,
                agent: None,
                approve: false,
            },
        )
        .unwrap();
        let store = SqliteStore::new(&crate::store::default_db_path()).unwrap();
        let before = store.operation_receipt("cli").unwrap().unwrap();
        let submitted = command(subdirectory, "cli");
        let (_, admitted) =
            super::admit("cli", &store.reading_identity().unwrap().1, &submitted).unwrap();
        assert_eq!(admitted.validation, OperationValidation::Candidate);
        assert_eq!(admitted.delivery, OperationDelivery::Untracked);
        super::execute(&store, "cli").unwrap();
        let finished = store.operation_receipt("cli").unwrap().unwrap();
        assert_eq!(finished.delivery, OperationDelivery::Finished);
        assert_eq!(
            finished.result,
            Some(serde_json::to_value(original).unwrap())
        );
        assert_eq!(finished.commits, before.commits);
        assert_eq!(store.list_tasks(None).unwrap().len(), 1);
        super::execute(&store, "cli").unwrap();
        assert_eq!(
            store.operation_receipt("cli").unwrap().unwrap().revision,
            finished.revision
        );
    }

    #[test]
    fn equivalent_creation_paths_validate_against_one_released_canonical_identity() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        let subdirectory = repo.path().join("nested");
        std::fs::create_dir(&subdirectory).unwrap();
        let link = tempfile::tempdir().unwrap();
        let symlink = link.path().join("repository");
        std::os::unix::fs::symlink(repo.path(), &symlink).unwrap();
        let canonical =
            super::creation_identity("old", &command(repo.path().to_path_buf(), "old")).unwrap();
        for directory in [repo.path().to_path_buf(), subdirectory, symlink] {
            let home = tempfile::tempdir().unwrap();
            let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
            store.claim_creation(&canonical).unwrap();
            let change = command(directory, "old");
            store
                .admit_operation(
                    &store.reading_identity().unwrap().1,
                    "old",
                    "task.create",
                    "submitted",
                    &serde_json::to_value(&change).unwrap(),
                )
                .unwrap();
            let saved = store.submitted_command("old").unwrap().unwrap();
            let resolved = super::creation_identity("old", &change).unwrap();
            assert_eq!(resolved.fingerprint, canonical.fingerprint);
            assert!(store
                .validate_operation("old", saved.generation, &resolved)
                .unwrap());
            assert_eq!(
                store.operation_receipt("old").unwrap().unwrap().fingerprint,
                Some(canonical.fingerprint.clone())
            );
        }
    }
}
