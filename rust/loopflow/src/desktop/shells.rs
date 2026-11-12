//! Desktop shell execution: create once, reattach, close, and retain history.
//! The Machine server's holder keeps the PTY; Session viewers alone hold Task
//! checkouts. This runtime needs no GUI on either execution Machine.
mod store;
pub(crate) use store::{hosts, jobs_settled};

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use clap::Args;
use serde::Deserialize;
use sha2::{Digest, Sha256};

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
        let launcher = crate::journal::current_lf_process_id();
        if store.create_desktop_shell(
            id,
            &checkout.task_id,
            &expected.to_string_lossy(),
            launcher.as_ref(),
        )? {
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
        // An earlier build's owner ends itself on the recorded Close.
        if let Some(host @ crate::transport::Host::Terminal(terminal)) = &receipt.host {
            if store.terminal_running(terminal)? {
                crate::transport::close_notice(host, "Shell closed")?;
            }
        }
        return Ok(());
    }

    let host = receipt.host.ok_or_else(|| {
        anyhow!(
            "Shell startup was not acknowledged; execution is unknown. No replacement was started"
        )
    })?;
    let ended = match &host {
        crate::transport::Host::Terminal(terminal) => !store.terminal_running(terminal)?,
        crate::transport::Host::Owner(owner) => {
            crate::journal::lf_evidence(&store, owner)
                == crate::journal::ProcessIdentityEvidence::Dead
        }
    };
    if ended {
        crate::transport::shell_history(&host)?;
        anyhow::bail!(
            "Shell ended. Create a new shell explicitly; its commands will not be replayed"
        );
    }
    crate::transport::attach(&host, None, None, None)
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
