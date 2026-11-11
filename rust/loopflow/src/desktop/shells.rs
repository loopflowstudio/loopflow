//! Desktop shell execution: create once, reattach, close, and retain history.
//! The shared transport owns the PTY and SSH stream; Session viewers alone hold
//! Task checkouts. This runtime needs no GUI on either execution Machine.
mod store;
pub(crate) use store::{jobs_settled, owners, request_retirement};

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::durable::TaskId;
use crate::id::LfProcessId;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

/// Private Desktop launch inputs, also carried verbatim across Machine routing.
#[derive(Args, Debug)]
pub struct ShellArgs {
    pub issue: String,
    #[arg(long)]
    pub checkout: std::path::PathBuf,
    #[arg(long)]
    pub id: String,
    /// Create once; retrying the identity never replays startup.
    #[arg(long, conflicts_with = "close")]
    pub create: bool,
    #[arg(long)]
    pub close: bool,
    /// Desktop startup script and relative UTF-8 resources, only for creation.
    #[arg(long, requires = "create")]
    pub bootstrap: Option<String>,
    #[arg(last = true)]
    pub command: Vec<String>,
}

pub fn run(args: &ShellArgs) -> Result<()> {
    let ShellArgs {
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
        let task = store
            .resolve_task_id(issue, None)?
            .ok_or_else(|| anyhow!("Task is unknown"))?;
        let checkout = store
            .task_checkout(&task)?
            .ok_or_else(|| anyhow!("Task has no checkout"))?;
        ensure!(
            checkout.machine_id.as_ref() == Some(&store.local_machine()?.id),
            "Task checkout belongs to another Machine; reopen the Task"
        );
        ensure!(
            checkout.worktree == *expected,
            "The Task checkout changed; reopen the Task. No shell was retargeted."
        );
        ensure!(
            expected.is_dir(),
            "The recorded Task checkout is unavailable"
        );
        let argv = shell_command(id, initial, bootstrap.as_deref())?;
        if store.create_desktop_shell(id, &checkout.task_id, &expected.to_string_lossy())? {
            crate::transport::start(&argv, true, expected, &|command| {
                command.env("LF_DESKTOP_SHELL", id);
            })?;
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
        return Ok(());
    }

    let owner = receipt.owner.ok_or_else(|| {
        anyhow!(
            "Shell startup was not acknowledged; execution is unknown. No replacement was started"
        )
    })?;
    if crate::journal::lf_evidence(&store, &owner) == crate::journal::ProcessIdentityEvidence::Dead
    {
        crate::transport::shell_history(&owner)?;
        anyhow::bail!(
            "Shell ended. Create a new shell explicitly; its commands will not be replayed"
        );
    }
    crate::transport::attach(&crate::transport::Host::Owner(owner), None, None, None)
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

/// Run inside the viewer observation's transaction, including brief return/leave.
pub(crate) fn cancel_retirement(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<()> {
    conn.execute(
        "UPDATE desktop_shells SET close_requested=0 WHERE task_id=?1 AND close_requested=2",
        [task.as_str()],
    )?;
    Ok(())
}

/// None is an ordinary Session owner; false keeps a Desktop shell alive.
pub(crate) fn close_allowed(
    store: &SqliteStore,
    owner: &LfProcessId,
    input_queued: bool,
) -> Result<Option<bool>> {
    let Some(close) = store.shell_close_request(owner)? else {
        return Ok(None);
    };
    let requested = match close {
        1 => true,
        2 => crate::ops::task::retirement::owner_retiring(store, owner)?,
        _ => false,
    };
    Ok(Some(
        requested
            && !input_queued
            && (!store.shell_holds_work(owner)? || store.owner_retention(owner)?.is_none()),
    ))
}

/// Whether work inside shell `owner` could receive typed input. An idle shell
/// clears any draft it recorded earlier; an unreadable store keeps recording.
pub(crate) fn holds_input(owner: &LfProcessId) -> bool {
    let holds = crate::journal::process_ledger().and_then(|store| {
        let holds = store.shell_holds_work(owner)?;
        if !holds {
            store.clear_owner_input(owner)?;
        }
        Ok(holds)
    });
    holds
        .inspect_err(
            |error| tracing::warn!(%error, %owner, "cannot judge shell work; retaining input"),
        )
        .unwrap_or(true)
}
