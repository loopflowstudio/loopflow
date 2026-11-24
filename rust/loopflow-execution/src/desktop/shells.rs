//! Desktop shell execution: create once, reattach, close, and retain history.
//! The Machine server's holder keeps the PTY; Session viewers alone hold Task
//! checkouts. This runtime needs no GUI on either execution Machine.
pub use loopflow_storage::sqlite::desktop_shells::{hosts, jobs_settled};

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Desktop shell operation inputs.
#[derive(Debug)]
pub struct ShellRequest {
    pub issue: String,
    pub checkout: std::path::PathBuf,
    pub id: String,
    /// Create once; retrying the identity never replays startup.
    pub create: bool,
    pub close: bool,
    /// Desktop startup script and relative UTF-8 resources, only for creation.
    pub bootstrap: Option<String>,
    pub command: Vec<String>,
}

pub fn run(args: &ShellRequest) -> Result<()> {
    let ShellRequest {
        issue,
        checkout: expected,
        id,
        create,
        close,
        command: initial,
        bootstrap,
    } = args;
    let store = crate::journal::process_ledger()?;
    // Only creation requires the recorded directory to exist. Existing receipts
    // remain attachable/closable after retirement, and only the insert winner
    // launches execution, including when concurrent creation loses its reply.
    if *create && store.desktop_shell(id)?.is_none() {
        let task = crate::store::children::resolve_task_id(&store, issue, None)?
            .ok_or_else(|| anyhow!("Task is unknown"))?;
        let checkout = store
            .task_checkout(&task)?
            .ok_or_else(|| anyhow!("Task has no checkout"))?;
        ensure!(
            checkout.worktree == *expected,
            "The Task checkout changed; reopen the Task. No shell was retargeted."
        );
        ensure!(
            expected.is_dir(),
            "The recorded Task checkout is unavailable"
        );
        if let Some(argv) = record(&store, id, &checkout.task_id, expected, initial, bootstrap)? {
            // A shell is a plain terminal: nothing in it is managed.
            let terminal = crate::transport::start_held(&argv, expected, None, &|_| {})?;
            store.bind_desktop_shell(id, &terminal)?;
        }
    }
    let receipt = store
        .desktop_shell(id)?
        .ok_or_else(|| anyhow!("Shell is unknown; create a new shell explicitly"))?;
    ensure!(
        receipt.checkout == expected.to_string_lossy(),
        "Shell belongs to another Task checkout"
    );
    if *close {
        store.close_desktop_shell(id)?;
        if let Some(terminal) = &receipt.terminal {
            if store.terminal_running(terminal)? {
                crate::transport::close_notice(terminal, "Shell closed")?;
            }
        }
        return Ok(());
    }

    ensure!(
        receipt.started,
        "Shell startup was not acknowledged; execution is unknown. No replacement was started"
    );
    // A shell an earlier build's owner ran is not viewed again.
    let running = match &receipt.terminal {
        Some(terminal) => store.terminal_running(terminal)?,
        None => false,
    };
    match receipt.terminal {
        Some(terminal) if running => crate::transport::attach(&terminal, None, Default::default()),
        terminal => {
            if let Some(terminal) = &terminal {
                crate::transport::shell_history(terminal)?;
            }
            anyhow::bail!(
                "Shell ended. Create a new shell explicitly; its commands will not be replayed"
            )
        }
    }
}

/// Record shell `id` for `task` in `checkout` and answer what to run, once:
/// `None` when the receipt exists, so a repeat never replays startup.
fn record(
    store: &loopflow_storage::sqlite::SqliteStore,
    id: &str,
    task: &loopflow_model::durable::TaskId,
    checkout: &Path,
    initial: &[String],
    bootstrap: &Option<String>,
) -> Result<Option<Vec<String>>> {
    let argv = shell_command(id, initial, bootstrap.as_deref())?;
    let launcher = crate::journal::current_lf_process_id();
    Ok(store
        .create_desktop_shell(id, task, &checkout.to_string_lossy(), launcher.as_ref())?
        .then_some(argv))
}

/// A Task shell this Machine's server holds, by `shell_id`.
pub mod served {
    use super::record;
    use crate::engine::{Error, ErrorCode};
    use crate::transport::registry::{Registry, Start};
    use loopflow_model::id::TerminalId;
    use serde_json::json;

    fn refused(error: impl Into<anyhow::Error>) -> Error {
        Error::refused(&error.into(), ErrorCode::Unavailable)
    }

    fn unknown(id: &str) -> Error {
        Error::new(
            ErrorCode::NotFound,
            "Shell is unknown here; start a new shell",
        )
        .with(json!({ "shell_id": id }))
    }

    /// Start shell `id` in the Task's checkout here, or answer the terminal
    /// an earlier start with the same `id` began. `env` is the server's.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        terminals: &Registry,
        issue: &str,
        id: &str,
        rows: u16,
        cols: u16,
        bootstrap: &Option<String>,
        command: &[String],
        env: Vec<(String, String)>,
    ) -> Result<TerminalId, Error> {
        let store = crate::journal::process_ledger().map_err(refused)?;
        if store.desktop_shell(id).map_err(refused)?.is_none() {
            let task = crate::store::children::resolve_task_id(&store, issue, None)
                .map_err(refused)?
                .ok_or_else(|| {
                    Error::new(ErrorCode::NotFound, format!("Task {issue} is unknown"))
                })?;
            let checkout = store
                .task_checkout(&task)
                .map_err(refused)?
                .ok_or_else(|| {
                    Error::new(ErrorCode::NotFound, format!("Task {issue} is not started"))
                        .with(json!({ "reason": "not_started", "task_id": task }))
                })?;
            let cwd = checkout.worktree;
            if let Some(argv) =
                record(&store, id, &task, &cwd, command, bootstrap).map_err(refused)?
            {
                let terminal = terminals
                    .start(Start {
                        session: None,
                        request: format!("shell-{id}"),
                        argv,
                        cwd,
                        env,
                        rows,
                        cols,
                        placement: None,
                    })
                    // The shell may be running: repeating the start answers it.
                    .map_err(|failure| {
                        Error::new(ErrorCode::OutcomeUnknown, format!("{failure:#}"))
                    })?;
                store.bind_desktop_shell(id, &terminal).map_err(refused)?;
                return Ok(terminal);
            }
        }
        let receipt = store
            .desktop_shell(id)
            .map_err(refused)?
            .ok_or_else(|| unknown(id))?;
        receipt.terminal.ok_or_else(|| {
            Error::new(
                ErrorCode::OutcomeUnknown,
                "Shell startup was not acknowledged; no replacement was started",
            )
            .with(json!({ "shell_id": id }))
        })
    }

    /// The running terminal of shell `id`; never starts one.
    pub fn running(id: &str) -> Result<TerminalId, Error> {
        let store = crate::journal::process_ledger().map_err(refused)?;
        let receipt = store
            .desktop_shell(id)
            .map_err(refused)?
            .ok_or_else(|| unknown(id))?;
        match receipt.terminal {
            Some(terminal) if store.terminal_running(&terminal).map_err(refused)? => Ok(terminal),
            terminal => Err(Error::new(
                ErrorCode::NotFound,
                "Shell ended; start a new shell. Its commands will not be replayed",
            )
            .with(json!({ "shell_id": id, "terminal_id": terminal }))),
        }
    }

    /// Close shell `id` and end its terminal.
    pub fn close(id: &str) -> Result<(), Error> {
        let store = crate::journal::process_ledger().map_err(refused)?;
        let receipt = store
            .desktop_shell(id)
            .map_err(refused)?
            .ok_or_else(|| unknown(id))?;
        store.close_desktop_shell(id).map_err(refused)?;
        if let Some(terminal) = &receipt.terminal {
            if store.terminal_running(terminal).map_err(refused)? {
                crate::transport::close_notice(terminal, "Shell closed").map_err(refused)?;
            }
        }
        Ok(())
    }
}

/// Desktop-supplied startup code and its relative UTF-8 resources. Carried as JSON
/// through Machine routing; no caller-local path is interpreted on the destination.
#[derive(Debug, Deserialize)]
struct ShellBootstrap {
    script: String,
    files: BTreeMap<String, String>,
}

fn shell_command(id: &str, initial: &[String], bootstrap: Option<&str>) -> Result<Vec<String>> {
    let mut script = String::new();
    let mut resources = String::new();
    if let Some(payload) = bootstrap {
        let bootstrap: ShellBootstrap = serde_json::from_str(payload)?;
        let directory = crate::store::lf_home_dir()
            .join("shell-resources")
            .join(format!("{:x}", Sha256::digest(payload.as_bytes())));
        for (name, contents) in bootstrap.files {
            ensure!(
                !name.is_empty()
                    && Path::new(&name)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_))),
                "Shell resource must be a relative file: {name}"
            );
            let destination = directory.join(name);
            if std::fs::read_to_string(&destination).ok().as_deref() == Some(&contents) {
                continue;
            }
            std::fs::create_dir_all(destination.parent().expect("resource has a directory"))?;
            let temporary = destination.with_extension(format!("{}.draft", uuid::Uuid::new_v4()));
            std::fs::write(&temporary, contents)?;
            std::fs::rename(temporary, destination)?;
        }
        script = bootstrap.script;
        resources = directory.to_string_lossy().into_owned();
    }
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let mut argv = vec![
        "/bin/sh".into(),
        "-c".into(),
        include_str!("shells/launch.sh").into(),
        id.into(),
        shell,
        script,
        resources,
    ];
    argv.extend_from_slice(initial);
    Ok(argv)
}
