//! Resolve retained execution before selecting the installed runtime that continues it.

use std::collections::BTreeSet;
use std::io::{Seek, Write};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;

use crate::child::ChildExecutionContext;
use crate::lf::commands::work_catalog::WorkCatalog;
use crate::lf::commands::WorkFilter;
use crate::machine_install;
use crate::ops::task::TaskLaunchOptions;

pub(super) fn destination() -> Result<Option<ChildExecutionContext>> {
    // Unit tests own ephemeral stores, never the account's real installation.
    #[cfg(test)]
    let root = match std::env::var_os("LF_TEST_TASK_INSTALL_ROOT") {
        Some(root) => std::path::PathBuf::from(root),
        None => return Ok(None),
    };
    #[cfg(not(test))]
    let root = machine_install::root()?;
    if machine_install::selection_for_executable(&root, &std::env::current_exe()?)?.is_some() {
        return Ok(None);
    }
    let Some(selection) = machine_install::current_selection(&root)? else {
        // Source-only machines execute against their private data. An installed
        // selection, when present, owns the executable and database together.
        return Ok(None);
    };
    Ok(Some(ChildExecutionContext {
        lf_bin: selection.verified_cli()?.to_path_buf(),
        lf_home: selection
            .store
            .parent()
            .context("installed store has no parent directory")?
            .to_path_buf(),
        db_path: selection.store,
    }))
}

/// Locate retained execution through installation receipts, without opening a
/// foreign store for migration or copying its records into the current store.
pub(crate) fn existing_task(issue: &str) -> Result<Option<ChildExecutionContext>> {
    existing_execution(|database| {
        let ids = task_ids(database, issue)?;
        Ok((!ids.is_empty()).then_some(ids))
    })
}

pub(crate) fn existing_session(id: &str) -> Result<Option<ChildExecutionContext>> {
    if let Some((task, _)) = id
        .split_once(':')
        .filter(|(task, _)| task.starts_with("task_"))
    {
        return existing_task(task);
    }
    existing_execution(|database| {
        let store = crate::store::sqlite::SqliteStore::open_read_only(database)?;
        let session = store.session(id)?.or(store.session_for_artifact(id)?);
        Ok(session.map(|session| {
            session
                .task_id
                .into_iter()
                .map(|id| id.to_string())
                .collect()
        }))
    })
}

fn existing_execution(
    contains: impl Fn(&Path) -> Result<Option<BTreeSet<String>>>,
) -> Result<Option<ChildExecutionContext>> {
    #[cfg(test)]
    let root = match std::env::var_os("LF_TEST_TASK_INSTALL_ROOT") {
        Some(root) => PathBuf::from(root),
        None => return Ok(None),
    };
    #[cfg(not(test))]
    let root = machine_install::root()?;
    let Some(selected) = machine_install::current_selection(&root)? else {
        return Ok(None);
    };
    let mut stores: Vec<PathBuf> = Vec::new();
    let mut found = Vec::new();
    for installation in machine_install::known_installations(&root)? {
        if stores.iter().any(|path| {
            crate::store::same_database_file(path, &installation.store).unwrap_or(false)
        }) {
            continue;
        }
        stores.push(installation.store.clone());
        if !installation.store.exists() {
            continue;
        }
        if let Some(tasks) = contains(&installation.store).with_context(|| {
            format!(
                "execution location {} could not be inspected",
                installation.store.display()
            )
        })? {
            found.push((installation.store, tasks));
        }
    }
    // A retained backup is not another execution owner until it has changed.
    // The source must also be present; provenance never manufactures missing work.
    let receipts = machine_install::retained_receipts(&root)?;
    let mut locations = found.iter().map(|(path, _)| path).collect::<Vec<_>>();
    for (database, tasks) in &found {
        if tasks.is_empty() {
            continue;
        }
        for receipt in &receipts {
            let (Some(source), Some(baseline)) = (&receipt.copied_from, &receipt.copied_tasks)
            else {
                continue;
            };
            if receipt.phase != machine_install::SwitchPhase::Settled
                || !crate::store::same_database_file(database, &receipt.target.store)?
                || !tasks.iter().all(|task| baseline.contains_key(task))
            {
                continue;
            }
            if !found.iter().any(|(path, ids)| {
                ids == tasks && crate::store::same_database_file(path, source).unwrap_or(false)
            }) {
                return Err(anyhow!(
                    "execution in {} was copied from {}; its source execution could not be located",
                    database.display(),
                    source.display()
                ));
            }
            let mut unchanged = true;
            for task in tasks {
                unchanged &=
                    machine_install::execution_copy::unchanged(database, task, &baseline[task])?;
            }
            if unchanged {
                locations.retain(|path| *path != database);
            }
        }
    }
    let database = match locations.as_slice() {
        [] if found.is_empty() => return Ok(None),
        [database] => *database,
        _ => {
            return Err(anyhow!(
                "execution exists in multiple distinct locations: {}",
                found
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    };
    let current = crate::store::database_path_from_env()?;
    if crate::store::same_database_file(database, &current)? {
        return Ok(None);
    }
    Ok(Some(ChildExecutionContext {
        lf_bin: selected.verified_cli()?.to_path_buf(),
        lf_home: database
            .parent()
            .context("execution database has no directory")?
            .to_path_buf(),
        db_path: database.clone(),
    }))
}

pub(crate) fn require_worker_destination() -> Result<()> {
    if let Some(destination) = destination()? {
        return Err(anyhow!("Task workers run through {} with data directory {}; run the Task through lf instead of invoking a branch worker", destination.lf_bin.display(), destination.lf_home.display()));
    }
    Ok(())
}

/// Compare stable identities without opening either schema for migration. A
/// branch-only Task (including a locally recovered successor) is not a transfer.
pub(super) fn check_task(context: &ChildExecutionContext, issue: &str) -> Result<()> {
    let local = task_ids(&crate::store::observability_database_path()?, issue)?;
    if !local.is_empty() && local != task_ids(&context.db_path, issue)? {
        return Err(anyhow!("Task {issue} has different identities in the branch and installed databases; its branch evidence remains private, and no Task was transferred"));
    }
    Ok(())
}

fn task_ids(path: &Path, issue: &str) -> Result<BTreeSet<String>> {
    Ok(WorkCatalog::load_at(path)?
        .owners
        .into_values()
        .filter(|owner| {
            owner.work.kind() == "task"
                && owner.matches(WorkFilter {
                    task: Some(issue),
                    wave: None,
                    project: None,
                })
        })
        .map(|owner| owner.work.id().to_string())
        .collect())
}

pub(super) fn launch_args(operation: &str, options: &TaskLaunchOptions) -> Vec<String> {
    let mut args = vec!["task".into(), operation.into()];
    for (flag, value) in [
        ("--model", &options.agent),
        ("--name", &options.name),
        ("--flow", &options.flow),
        ("--stack-on", &options.stack_on),
        ("--directive", &options.directive),
        ("--reason", &options.reason),
    ] {
        if let Some(value) = value {
            args.extend([flag.into(), value.clone()]);
        }
    }
    if options.retry {
        args.push("--retry".into());
    }
    args
}

pub(super) fn json<T: DeserializeOwned>(
    context: &ChildExecutionContext,
    cwd: &Path,
    mut args: Vec<String>,
    input: Option<&str>,
) -> Result<T> {
    args.push("--json".into());
    serde_json::from_slice(&execute(context, cwd, &args, input)?)
        .context("read installed Task operation result")
}

fn command(context: &ChildExecutionContext, cwd: &Path, args: &[String]) -> Command {
    let mut command = Command::new(&context.lf_bin);
    command.current_dir(cwd).args(args).stderr(Stdio::inherit());
    // A different data copy cannot inherit Run, worker, account or switch authority.
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LF_") {
            command.env_remove(name);
        }
    }
    if let Some(declaration) = std::env::var_os(crate::lf::WORK_DECLARATION_ENV) {
        command.env(crate::lf::WORK_DECLARATION_ENV, declaration);
    }
    for name in ["LF_HOME", "LF_CONTROL_HOME"] {
        command.env(name, &context.lf_home);
    }
    for name in ["LF_DB_PATH", "LF_CONTROL_DB_PATH"] {
        command.env(name, &context.db_path);
    }
    for name in ["LF_BIN", "LF_CONTROL_BIN"] {
        command.env(name, &context.lf_bin);
    }
    command
}

/// Continue in the selected runtime without detaching the caller's terminal.
pub(crate) fn forward_session(
    context: &ChildExecutionContext,
    cwd: &Path,
    args: &[String],
    readiness: bool,
) -> Result<()> {
    let mut command = command(context, cwd, args);
    if readiness {
        // Preserve the exact review's capture token; the owning store fences it.
        for name in [
            crate::durable::RUN_ID_ENV,
            crate::ops::human_session::HUMAN_SESSION_ENV,
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
    }
    #[cfg(unix)]
    {
        Err(command.exec()).context("continue Session through installed lf")
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .context("continue Session through installed lf")?;
        anyhow::ensure!(
            status.success(),
            "installed Session operation failed ({status})"
        );
        Ok(())
    }
}

pub(super) fn execute(
    context: &ChildExecutionContext,
    cwd: &Path,
    args: &[String],
    input: Option<&str>,
) -> Result<Vec<u8>> {
    let mut command = command(context, cwd, args);
    // A file avoids pipe capacity limits for large Task reports.
    if let Some(input) = input {
        let mut stdin = tempfile::tempfile()?;
        stdin.write_all(input.as_bytes())?;
        stdin.rewind()?;
        command.stdin(stdin);
    } else {
        command.stdin(Stdio::null());
    }
    #[cfg(unix)]
    command.process_group(0);
    let child = command
        .stdout(Stdio::piped())
        .spawn()
        .context("run installed Task operation")?;
    let guard = crate::engine::process::ProcessGroupGuard::new(child.id());
    let output = child
        .wait_with_output()
        .context("wait for installed Task operation")?;
    guard.disarm();
    if !output.status.success() {
        return Err(anyhow!(
            "installed Task operation failed ({}) with data directory {}",
            output.status,
            context.lf_home.display()
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use rusqlite::Connection;

    use super::destination;
    use crate::engine::process::shell_escape;
    use crate::machine_install::{
        artifact_set_sha256, write_active, ActiveInstall, ArtifactIdentity, ArtifactRole,
        ArtifactSet, InstallSelection, InstallSource,
    };
    use crate::ops::task::{
        task_create, task_restart, task_run, TaskCreateResult, TaskLaunchOptions,
    };

    const TEST: &str =
        "ops::task_destination::tests::managed_operations_move_before_branch_effects";

    fn identity_store(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE waves(id TEXT, name TEXT, created_at INTEGER);
            CREATE TABLE projects(id TEXT, wave_id TEXT, project_slug TEXT, external_project_id TEXT, created_at INTEGER);
            CREATE TABLE tasks(id TEXT, project_id TEXT, issue_identifier TEXT, external_issue_id TEXT, created_at INTEGER);
            CREATE TABLE operations(name TEXT, executable TEXT, home TEXT);").unwrap();
        conn
    }

    #[test]
    fn managed_operations_move_before_branch_effects() {
        match std::env::var("TASK_DESTINATION_CASE").as_deref() {
            Ok("installed") => {
                // Simulate an older installed operation: it rejects the branch's
                // newer schema and writes only to its explicitly selected store.
                let db = PathBuf::from(std::env::var_os("LF_DB_PATH").unwrap());
                assert_eq!(
                    db,
                    PathBuf::from(std::env::var_os("LF_CONTROL_DB_PATH").unwrap())
                );
                assert_eq!(
                    std::env::var_os("LF_HOME"),
                    std::env::var_os("LF_CONTROL_HOME")
                );
                assert!(std::env::var_os("LF_WORK_ADVANCE_CLAIM").is_none());
                assert!(std::env::var_os("LF_RUN_ID").is_none());
                assert!(std::env::var_os("LF_ACCOUNT_LEASE").is_none());
                let conn = Connection::open(db).unwrap();
                let drafts: i64 = conn
                    .query_row(
                        "SELECT count(*) FROM sqlite_master WHERE name='branch_draft'",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(drafts, 0);
                conn.execute(
                    "INSERT INTO operations VALUES (?1, ?2, ?3)",
                    (
                        std::env::var("TASK_DESTINATION_OPERATION").unwrap(),
                        std::env::var("LF_BIN").unwrap(),
                        std::env::var("LF_HOME").unwrap(),
                    ),
                )
                .unwrap();
                return;
            }
            Ok("driver") => {}
            _ => {
                let output = Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", TEST, "--nocapture"])
                    .env_clear()
                    .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                    .env("TASK_DESTINATION_CASE", "driver")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                return;
            }
        }
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let home = root.join("installed");
        let branch = root.join("branch");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&branch).unwrap();
        let installed_db = home.join("loopflow.db");
        let branch_db = branch.join("loopflow.db");
        let installed = identity_store(&installed_db);
        let local = identity_store(&branch_db);
        local.execute_batch("CREATE TABLE branch_draft(value TEXT); INSERT INTO branch_draft VALUES ('private write');").unwrap();
        let task_id = crate::work::task::TaskId::new();
        let snapshot = crate::ops::task::TaskSnapshot {
            issue_id: "issue-installed".into(),
            issue_identifier: "LOO-1".into(),
            task_id: task_id.to_string(),
            external_project_id: "project-external".into(),
            project: "installed".into(),
            pm_snapshot_synced_at: 1,
            pm_writeback: crate::work::task::PmWritebackState::Current,
            wave: "installed".into(),
            project_id: "project_installed".into(),
            status: crate::durable::WorkStatus::Ready,
            execution: crate::ops::task_execution::TaskExecutionSnapshot {
                state: crate::ops::task_execution::TaskExecutionState::Idle,
                reason: "simulated installed operation".into(),
                step: None,
                captured: None,
            },
            runs: vec![],
            runs_truncated: false,
            worktree: root.display().to_string(),
            workspace_slug: "fixture".into(),
            agent: None,
            provider: "fixture".into(),
            prs: vec![],
            active_pr: None,
            latest_event: None,
            created_at: time::OffsetDateTime::now_utc(),
            updated_at: time::OffsetDateTime::now_utc(),
            observation: crate::work::task::Observation::NotRequired,
            actions: crate::ops::task_actions::TaskActionModel::no_task(),
        };
        fs::write(
            home.join("task.json"),
            serde_json::to_vec(&snapshot).unwrap(),
        )
        .unwrap();
        let cli = root.join("installed-lf");
        fs::write(&cli, format!("#!/bin/sh\nexport TASK_DESTINATION_CASE=installed\nexport TASK_DESTINATION_OPERATION=\"$2\"\n{} --exact {} --nocapture 1>&2 || exit 99\nif [ -n \"$TASK_DESTINATION_FAIL\" ]; then exit 19; fi\ncat \"$LF_HOME/task.json\"\n", shell_escape(&std::env::current_exe().unwrap().to_string_lossy()), TEST)).unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        let artifacts = ArtifactSet {
            id: "fixture".into(),
            source: InstallSource::Development,
            source_revision: "fixture".into(),
            source_identity: "fixture".into(),
            content_sha256: artifact_set_sha256(&cli, None, None).unwrap(),
            artifacts: vec![ArtifactIdentity::capture(ArtifactRole::Cli, &cli).unwrap()],
        };
        let mut published = artifacts.clone();
        published.source = InstallSource::Published;
        let machine = root.join("machine");
        std::env::set_var("LF_TEST_TASK_INSTALL_ROOT", &machine);
        assert!(destination().unwrap().is_none());
        assert!(super::require_worker_destination().is_ok());
        write_active(
            &machine,
            &ActiveInstall {
                schema_version: 1,
                selection: InstallSelection {
                    installation_id: "fixture".into(),
                    source: InstallSource::Development,
                    artifact_set: artifacts,
                    store: installed_db.clone(),
                },
                published_fallback: published.clone(),
                retained_published_sets: vec![published],
            },
        )
        .unwrap();
        for name in ["LF_HOME", "LF_CONTROL_HOME"] {
            std::env::set_var(name, &branch);
        }
        for name in ["LF_DB_PATH", "LF_CONTROL_DB_PATH"] {
            std::env::set_var(name, &branch_db);
        }
        std::env::set_var("LF_WORK_ADVANCE_CLAIM", "foreign claim");
        std::env::set_var("LF_RUN_ID", "foreign run");
        std::env::set_var("LF_ACCOUNT_LEASE", "foreign lease");
        let before = fs::read(&branch_db).unwrap();
        // No Git checkout, PM account or usable execution schema exists here.
        // Every operation must reach installation before needing any of them.
        assert_eq!(
            task_run(&root, "LOO-1", TaskLaunchOptions::default()).unwrap(),
            snapshot
        );
        let TaskCreateResult::Started(created) = task_create(
            &root,
            None,
            Some("Title".into()),
            Some("Report".into()),
            Some(TaskLaunchOptions::default()),
        )
        .unwrap() else {
            panic!("create --run must return the installed snapshot")
        };
        assert_eq!(*created, snapshot);
        assert_eq!(task_restart("LOO-1", None, None, None).unwrap(), snapshot);
        assert_eq!(fs::read(&branch_db).unwrap(), before);
        let operations: Vec<(String, String, String)> = installed
            .prepare("SELECT name, executable, home FROM operations ORDER BY rowid")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            operations
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            ["run", "create", "restart"]
        );
        for (_, executable, selected_home) in operations {
            assert_eq!(Path::new(&executable), cli);
            assert_eq!(Path::new(&selected_home), home);
        }
        assert!(super::require_worker_destination().is_err());
        // A Task that exists only in this branch cannot become an installed Task.
        local.execute_batch("INSERT INTO waves VALUES ('00000000-0000-0000-0000-000000000001', 'local', 1);
            INSERT INTO projects VALUES ('project_local', '00000000-0000-0000-0000-000000000001', 'local', 'project-external', 1);
            INSERT INTO tasks VALUES ('task_local', 'project_local', 'LOO-2', 'issue-local', 1);").unwrap();
        assert!(task_run(&root, "LOO-2", TaskLaunchOptions::default())
            .unwrap_err()
            .to_string()
            .contains("no Task was transferred"));
        assert_eq!(
            installed
                .query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            installed
                .query_row("SELECT count(*) FROM operations", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            local
                .query_row("SELECT value FROM branch_draft", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "private write"
        );
        std::env::set_var("TASK_DESTINATION_FAIL", "1");
        let before_failure = fs::read(&branch_db).unwrap();
        let error = task_restart("LOO-1", None, None, None).unwrap_err();
        assert!(error
            .to_string()
            .contains("installed Task operation failed"));
        assert_eq!(fs::read(&branch_db).unwrap(), before_failure);
        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(crate::controller::task::run_worker(task_id))
            .unwrap_err();
        assert!(error.to_string().contains("Task workers run through"));
        assert_eq!(fs::read(&branch_db).unwrap(), before_failure);

        // A selected runtime cannot silently fall back to an inherited binary
        // when its recorded bytes change or disappear.
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        assert_eq!(
            crate::engine::process::resolve_current_home_lf_binary_checked().unwrap(),
            cli
        );
        fs::write(&cli, "changed installed bytes").unwrap();
        assert!(crate::engine::process::resolve_current_home_lf_binary_checked().is_err());
        fs::remove_file(&cli).unwrap();
        assert!(crate::engine::process::resolve_current_home_lf_binary_checked().is_err());
    }
}
