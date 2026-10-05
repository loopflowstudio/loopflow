//! Process-local observations, never Session authority. Append each phase directly
//! to the Home so a busy SQLite writer cannot hide a connection's timing.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};
use serde_json::json;

use crate::id::ExecId;

static ACTIVE: Mutex<Option<ConnectionTiming>> = Mutex::new(None);

#[derive(Debug)]
struct ConnectionTiming {
    file: File,
    exec: ExecId,
    selector: String,
    session: Option<String>,
    attached_at: Option<std::time::Duration>,
}

fn path(exec: &ExecId) -> PathBuf {
    crate::store::lf_home_dir()
        .join("runtime/session-connect")
        .join(format!("{exec}.jsonl"))
}

pub(crate) fn start(selector: &str) -> Result<()> {
    let exec = super::current_exec_id().context("connection has no Exec")?;
    let path = path(&exec);
    fs::create_dir_all(path.parent().expect("timing file has a directory"))?;
    let file = OpenOptions::new().create_new(true).write(true).open(path)?;
    *ACTIVE.lock().expect("connection timing mutex poisoned") = Some(ConnectionTiming {
        file,
        exec,
        selector: selector.into(),
        session: None,
        attached_at: None,
    });
    crate::engine::agent::register_interrupt_cleanup(|| finish("interrupted"));
    phase("started");
    unavailable(
        "first_output",
        "native UI owns stdout; use the external PTY runner",
    );
    unavailable(
        "input_ready",
        "native UI exposes no input-readiness event; input acceptance is an upper bound",
    );
    Ok(())
}

pub(crate) fn resolved(session: &str) {
    if let Some(active) = ACTIVE
        .lock()
        .expect("connection timing mutex poisoned")
        .as_mut()
    {
        active.session = Some(session.into());
    }
    phase("lookup_complete");
}

pub(crate) fn phase(name: &str) {
    record(name, None);
}

fn unavailable(name: &str, reason: &str) {
    record(name, Some(reason));
}

fn record(phase: &str, unavailable: Option<&str>) {
    let mut active = ACTIVE.lock().expect("connection timing mutex poisoned");
    let Some(active) = active.as_mut() else {
        return;
    };
    let elapsed = unavailable.is_none().then(super::process_elapsed).flatten();
    if phase == "attached" {
        active.attached_at = elapsed;
    }
    let duration = if matches!(phase, "exited" | "failed" | "interrupted") {
        elapsed
            .zip(active.attached_at)
            .map(|(end, start)| end.saturating_sub(start))
    } else {
        None
    };
    let record = json!({
        "exec": active.exec,
        "selector": active.selector,
        "session": active.session,
        "phase": phase,
        "elapsed_ms": elapsed.map(|elapsed| elapsed.as_secs_f64() * 1000.0),
        "attached_lifetime_ms": duration.map(|duration| duration.as_secs_f64() * 1000.0),
        "unavailable": unavailable,
    });
    if let Err(error) = writeln!(active.file, "{record}").and_then(|()| active.file.flush()) {
        tracing::warn!(%error, "cannot write Session connection timing");
    }
}

pub(crate) fn finish(outcome: &str) {
    phase(outcome);
    ACTIVE
        .lock()
        .expect("connection timing mutex poisoned")
        .take();
}

pub(crate) fn read(exec: &ExecId) -> Result<String> {
    fs::read_to_string(path(exec)).with_context(|| format!("no connection timings for Exec {exec}"))
}
