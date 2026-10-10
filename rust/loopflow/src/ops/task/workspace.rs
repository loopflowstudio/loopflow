//! Retained-checkout companions. Neither operation prepares or starts Task work.
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

use anyhow::{anyhow, ensure, Result};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use super::{file_context, file_store};
use crate::durable::{MachineId, TaskId};
use crate::store::sqlite::TaskCheckout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFilesFrame {
    pub request: String,
    pub task_id: TaskId,
    pub machine_id: MachineId,
    pub checkout: String,
    pub changed: bool,
}

fn pinned_checkout(issue: &str, expected: &Path) -> Result<TaskCheckout> {
    let checkout = file_context(&file_store()?, issue)?;
    ensure!(
        checkout.worktree == expected,
        "The Task checkout changed; reopen the Task. No companion was retargeted."
    );
    ensure!(
        expected.is_dir(),
        "The recorded Task checkout is unavailable"
    );
    Ok(checkout)
}

pub fn task_shell(issue: &str, expected: &Path) -> Result<()> {
    let checkout = pinned_checkout(issue, expected)?;
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    let status = std::process::Command::new(shell)
        .arg("-l")
        .current_dir(checkout.worktree)
        .status()?;
    if !status.success() {
        return Err(crate::process::CommandExit(status.code().unwrap_or(1) as u8).into());
    }
    Ok(())
}

/// The existing owned-line transport keeps stdin open and closes it on cancellation.
/// Heartbeats recheck recorded ownership; filesystem events, not a read timer,
/// invalidate contents. Access events from our own reads never cause another read.
pub fn watch_task_files(issue: &str, expected: &Path, request: &str) -> Result<()> {
    let checkout = pinned_checkout(issue, expected)?;
    let (wake, events) = mpsc::sync_channel(1);
    let error = Arc::new(Mutex::new(None));
    let failure = error.clone();
    let changes = wake.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(event) if event.kind.is_access() => return,
            Ok(_) => {}
            Err(error) => {
                *failure.lock().expect("file observation error lock") = Some(error.to_string())
            }
        }
        // One outstanding invalidation represents every event until the next read.
        let _ = changes.try_send(());
    })?;
    watcher.watch(expected, RecursiveMode::Recursive)?;
    let metadata = crate::engine::git::absolute_git_dir(expected)?;
    if !metadata.starts_with(expected) {
        watcher.watch(&metadata, RecursiveMode::Recursive)?;
    }
    let closed = Arc::new(AtomicBool::new(false));
    let input_closed = closed.clone();
    std::thread::spawn(move || {
        let mut byte = [0];
        while std::io::stdin().read(&mut byte).is_ok_and(|n| n != 0) {}
        input_closed.store(true, Ordering::Release);
        let _ = wake.try_send(());
    });
    let mut changed = true;
    loop {
        if closed.load(Ordering::Acquire) {
            return Ok(());
        }
        if let Some(error) = error.lock().expect("file observation error lock").take() {
            return Err(anyhow!("Live file observation failed: {error}"));
        }
        let current = pinned_checkout(issue, expected)?;
        ensure!(
            current.task_id == checkout.task_id && current.machine_id == checkout.machine_id,
            "Task execution ownership changed; reopen the Task"
        );
        let frame = TaskFilesFrame {
            request: request.into(),
            task_id: current.task_id,
            machine_id: current.machine_id.expect("file_context requires a Machine"),
            checkout: expected.to_string_lossy().into_owned(),
            changed,
        };
        println!("{}", serde_json::to_string(&frame)?);
        std::io::stdout().flush()?;
        changed = events.recv_timeout(Duration::from_secs(2)).is_ok();
        if changed {
            std::thread::sleep(Duration::from_millis(150));
            while events.try_recv().is_ok() {}
        }
    }
}
