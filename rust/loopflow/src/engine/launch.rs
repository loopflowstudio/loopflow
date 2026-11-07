//! Start an `lf` execution detached from its caller and wait for its result.
//!
//! A served request has no Process to carry a Task run or a ship, and must not
//! hold one open. The operation starts the same command a terminal would,
//! under an execution owner of its own, with `LF_LAUNCH_REPORT=<path>`. The
//! command writes its typed result there once it knows it, as its CLI prints
//! it with `--json`: when its Graph registers, when it refused, or when there
//! was nothing to run. The command then goes on without its launcher.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{Error, ErrorCode, Result};
use crate::id::LfProcessId;

const REPORT_ENV: &str = "LF_LAUNCH_REPORT";
static REPORT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// What a detached operation set going: the `lf` Process that carries it.
/// `process.get` reads how it is going; `log` is its output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Launched {
    /// The operation the Process carries: `checkout.sync`, `pr.land`,
    /// `repo.release`, `graph.run`, `skill.run`.
    pub operation: String,
    #[serde(rename = "lf_process_id")]
    pub process: LfProcessId,
    /// What it set out to do, for messages: `sync onto origin/main`.
    pub run: String,
    /// Where its output goes, now that its launcher has let go.
    pub log: PathBuf,
}

impl Launched {
    pub fn render(&self) -> String {
        format!(
            "{} running as process {}\n  follow: lf process show {}\n  log:    {}",
            self.run,
            self.process,
            self.process,
            self.log.display()
        )
    }
}

/// Whether a launcher waits for this process's report.
pub fn expected() -> bool {
    REPORT.lock().expect("report path poisoned").is_some()
}

/// This process carries `operation` for the launcher that started it
/// detached: report the Process, then let the launcher go and keep this
/// process's output in a log of its own. None when no launcher waits: the
/// command then runs in its terminal as it always has.
pub(crate) fn carry(operation: &str, run: impl Into<String>) -> anyhow::Result<Option<Launched>> {
    if !expected() {
        return Ok(None);
    }
    let process = crate::journal::current_lf_process_id()
        .context("a detached operation requires a registered Process")?;
    let log = crate::lf::commands::graph::PendingLog::open(&format!(
        "{}-{process}",
        operation.replace('.', "-")
    ))?;
    let launched = Launched {
        operation: operation.to_owned(),
        process,
        run: run.into(),
        log: log.path.clone(),
    };
    report(Ok(&launched));
    crate::lf::commands::graph::release_launcher(&log.file)?;
    Ok(Some(launched))
}

/// Take the report path this process's launcher passed. The execution owner
/// between them passes it on; this process's own children never inherit it.
pub fn expect() {
    if let Some(path) = std::env::var_os(REPORT_ENV) {
        std::env::remove_var(REPORT_ENV);
        *REPORT.lock().expect("report path poisoned") = Some(path.into());
    }
}

/// Write this process's result for its launcher, if one asked and nothing was
/// reported yet. Later reports are ignored: the launcher has gone.
pub fn report<T: Serialize>(result: std::result::Result<&T, &anyhow::Error>) {
    let Some(path) = REPORT.lock().expect("report path poisoned").take() else {
        return;
    };
    let result = match result {
        Ok(value) => serde_json::to_value(value)
            .map_err(|error| Error::written(&error.into()))
            .map(Ok),
        Err(error) => Ok(Err(Error::written(error))),
    }
    .unwrap_or_else(Err);
    if let Err(error) = write(&path, &result) {
        tracing::warn!(path = %path.display(), %error, "Could not report the launched command's result");
    }
}

fn write(
    path: &Path,
    result: &std::result::Result<serde_json::Value, Error>,
) -> anyhow::Result<()> {
    let partial = path.with_extension("partial");
    std::fs::write(&partial, serde_json::to_vec(result)?)?;
    std::fs::rename(&partial, path)?;
    Ok(())
}

/// Start `lf <args>` in `directory` under its own execution owner and return
/// the result it reports. No deadline: the owner's exit bounds the wait.
pub(crate) fn start<T: DeserializeOwned>(directory: &Path, args: &[String]) -> Result<T> {
    let launched = (|| -> anyhow::Result<_> {
        let path = crate::transport::private_directory()?
            .join(format!("report-{}.json", uuid::Uuid::new_v4()));
        let mut argv = vec![crate::os_process::execution_context()?
            .lf_bin
            .to_string_lossy()
            .into_owned()];
        argv.extend(args.iter().cloned());
        let report = path.to_string_lossy().into_owned();
        let owner = crate::os_process::start_lf_session(
            directory,
            &argv,
            &[(REPORT_ENV, report.as_str())],
            &|_| {},
        )?;
        Ok((path, owner))
    })();
    let (path, owner) = launched.map_err(|error| Error::outcome_unknown(&error))?;
    let registry = crate::store::read_existing_registry()
        .ok()
        .flatten()
        .map(|store| store.sqlite);
    loop {
        if let Some(result) = read(&path)? {
            return result;
        }
        let ended = registry.as_ref().is_some_and(|store| {
            crate::journal::lf_evidence(store, &owner)
                == crate::journal::ProcessIdentityEvidence::Dead
        });
        if ended {
            // It may have written just before it exited.
            return read(&path)?.unwrap_or_else(|| {
                Err(Error::new(
                    ErrorCode::OutcomeUnknown,
                    format!(
                        "`lf {}` ended without a result; inspect the affected work before retrying",
                        args.join(" ")
                    ),
                ))
            });
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<Result<T>>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::outcome_unknown(&error.into())),
    };
    let _ = std::fs::remove_file(path);
    let reported: std::result::Result<serde_json::Value, Error> =
        serde_json::from_slice(&bytes).map_err(|error| Error::outcome_unknown(&error.into()))?;
    Ok(Some(reported.and_then(|value| {
        serde_json::from_value(value).map_err(|error| Error::outcome_unknown(&error.into()))
    })))
}
