//! Start an `lf` execution detached from its caller and wait for its result.
//!
//! A served request has no Process to carry a Task run or a ship, and must not
//! hold one open. The operation starts the same command a terminal would,
//! as a detached lf process of its own, with `LF_LAUNCH_REPORT=<path>`. The
//! command writes its typed result there once it knows it, as its CLI prints
//! it with `--json`: when its Graph registers, when it refused, or when there
//! was nothing to run. The command then goes on without its launcher.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{Error, ErrorCode, Result};
use crate::id::LfProcessId;
use crate::journal::ProcessIdentityEvidence;

const REPORT_ENV: &str = "LF_LAUNCH_REPORT";
const UNKNOWN_GRACE: Duration = Duration::from_secs(5);
static REPORT: Mutex<Option<PathBuf>> = Mutex::new(None);
const CREATION_ENV: &str = "LF_CREATION_REQUEST";
static CREATION: Mutex<Option<String>> = Mutex::new(None);

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

/// Take the report path this process's launcher passed. The detached
/// launch passes it on; this process's own children never inherit it.
pub fn expect() {
    if let Ok(id) = std::env::var(CREATION_ENV) {
        std::env::remove_var(CREATION_ENV);
        *CREATION.lock().expect("creation request poisoned") = Some(id);
    }
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
    if let Some(id) = CREATION.lock().expect("creation request poisoned").take() {
        if let Ok(value) = &result {
            let saved = (|| -> anyhow::Result<()> {
                let store = crate::store::read_existing_registry()?
                    .context("Machine registry is unavailable")?;
                store.sqlite.record_creation_launch(&id, value)?;
                Ok(())
            })();
            if let Err(error) = saved {
                tracing::warn!(%error, request_id = %id, "Could not record creation launch");
            }
        }
    }
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

/// Start `lf <args>` in `directory` as a plain detached process and return
/// the result it reports. Wait while that process is live, but bound the wait when
/// its liveness cannot be determined. Giving up never stops or retries the work.
pub(crate) fn start<T: DeserializeOwned>(directory: &Path, args: &[String]) -> Result<T> {
    start_reported(directory, args, None)
}

pub(crate) fn start_creation<T: DeserializeOwned>(
    directory: &Path,
    args: &[String],
    request: &str,
) -> Result<T> {
    start_reported(directory, args, Some(request))
}

fn start_reported<T: DeserializeOwned>(
    directory: &Path,
    args: &[String],
    creation: Option<&str>,
) -> Result<T> {
    let launched = (|| -> anyhow::Result<_> {
        let path = crate::transport::private_directory()?
            .join(format!("report-{}.json", uuid::Uuid::new_v4()));
        let mut argv = vec![crate::os_process::execution_context()?
            .lf_bin
            .to_string_lossy()
            .into_owned()];
        argv.extend(args.iter().cloned());
        let report = path.to_string_lossy().into_owned();
        let mut environment = vec![(REPORT_ENV, report.as_str())];
        if let Some(id) = creation {
            environment.push((CREATION_ENV, id));
        }
        let process =
            crate::os_process::detached::start_lf(directory, &argv, &environment, &|_| {})?;
        Ok((path, process))
    })();
    let (path, process) = launched.map_err(|error| Error::outcome_unknown(&error))?;
    wait(&path, args, || process_evidence(&process), SystemTime::now)
}

fn process_evidence(process: &LfProcessId) -> ProcessIdentityEvidence {
    crate::store::read_existing_registry()
        .ok()
        .flatten()
        .map_or(ProcessIdentityEvidence::Unknown, |store| {
            crate::journal::lf_evidence(&store.sqlite, process)
        })
}

fn wait<T: DeserializeOwned>(
    path: &Path,
    args: &[String],
    mut observe: impl FnMut() -> ProcessIdentityEvidence,
    mut now: impl FnMut() -> SystemTime,
) -> Result<T> {
    let mut unknown_since = None;
    loop {
        if let Some(result) = read(path)? {
            return result;
        }
        let reason = match observe() {
            ProcessIdentityEvidence::Live => {
                unknown_since = None;
                None
            }
            ProcessIdentityEvidence::Dead => Some("ended without a result"),
            ProcessIdentityEvidence::Unknown => {
                let now = now();
                let since = *unknown_since.get_or_insert(now);
                (now.duration_since(since).unwrap_or_default() >= UNKNOWN_GRACE)
                    .then_some("has not reported a result and its liveness could not be determined")
            }
        };
        if let Some(reason) = reason {
            // It may have written while we checked its liveness.
            return read(path)?.unwrap_or_else(|| {
                Err(Error::new(
                    ErrorCode::OutcomeUnknown,
                    format!(
                        "`lf {}` {reason}; inspect the affected work before retrying",
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
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| Error::outcome_unknown(&error.into()))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{process_evidence, wait, write, UNKNOWN_GRACE};
    use crate::engine::{Error, ErrorCode};
    use crate::id::LfProcessId;
    use crate::journal::{ProcessIdentityEvidence, TestLedgerGuard};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn unknown_liveness_returns_an_uncertain_outcome_after_the_grace() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let mut times = [
            SystemTime::UNIX_EPOCH,
            SystemTime::UNIX_EPOCH + UNKNOWN_GRACE,
        ]
        .into_iter();
        let error = wait::<String>(
            &path,
            &["task".into(), "run".into(), "LOO-804".into()],
            || ProcessIdentityEvidence::Unknown,
            || times.next().expect("unknown liveness must stop waiting"),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OutcomeUnknown);
        assert!(error.message.contains("lf task run LOO-804"));
        assert!(error.message.contains("liveness could not be determined"));
        assert!(error
            .message
            .contains("inspect the affected work before retrying"));
    }

    #[test]
    fn live_process_resets_the_unknown_grace_and_can_report_later() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let mut observations = [
            ProcessIdentityEvidence::Unknown,
            ProcessIdentityEvidence::Live,
            ProcessIdentityEvidence::Unknown,
            ProcessIdentityEvidence::Unknown,
        ]
        .into_iter();
        let mut seconds = [0, 10, 14].into_iter();
        let result: String = wait(
            &path,
            &[],
            || observations.next().unwrap(),
            || {
                let second = seconds.next().unwrap();
                if second == 14 {
                    write(&path, &Ok(serde_json::json!("started"))).unwrap();
                }
                SystemTime::UNIX_EPOCH + Duration::from_secs(second)
            },
        )
        .unwrap();
        assert_eq!(result, "started");
        assert!(!path.exists());
    }

    #[test]
    fn reports_win_over_exit_and_expired_unknown_liveness() {
        for evidence in [
            ProcessIdentityEvidence::Dead,
            ProcessIdentityEvidence::Unknown,
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("report.json");
            let mut observations = 0;
            let mut time = SystemTime::UNIX_EPOCH;
            let result: String = wait(
                &path,
                &[],
                || {
                    observations += 1;
                    if evidence == ProcessIdentityEvidence::Dead || observations == 2 {
                        write(&path, &Ok(serde_json::json!("started"))).unwrap();
                    }
                    evidence
                },
                || {
                    time += UNKNOWN_GRACE;
                    time
                },
            )
            .unwrap();
            assert_eq!(result, "started");
            assert!(!path.exists());
        }
    }

    #[test]
    fn a_reported_refusal_returns_without_observing_the_process() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let refusal = Error::new(ErrorCode::Conflict, "already running");
        write(&path, &Err(refusal.clone())).unwrap();
        let error = wait::<String>(
            &path,
            &[],
            || panic!("the report already answered"),
            SystemTime::now,
        )
        .unwrap_err();
        assert_eq!(error, refusal);
    }

    #[test]
    fn malformed_reports_return_an_uncertain_outcome() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        for report in ["not json", r#"{"Ok":42}"#] {
            std::fs::write(&path, report).unwrap();
            let error = wait::<String>(
                &path,
                &[],
                || panic!("an unreadable report must stop waiting"),
                SystemTime::now,
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OutcomeUnknown);
            assert!(!path.exists());
        }
    }

    #[test]
    fn polling_recovers_when_a_missing_or_unreadable_registry_returns() {
        for unreadable in [false, true] {
            let ledger = TestLedgerGuard::new();
            let database = ledger.home().join("loopflow.db");
            let path = ledger.home().join("report.json");
            let process = LfProcessId::new();
            if unreadable {
                std::fs::write(&database, "not a database").unwrap();
            }
            let mut recovered = false;
            let error = wait::<String>(
                &path,
                &[],
                || process_evidence(&process),
                || {
                    assert!(!recovered, "the next poll must see the finished process");
                    recovered = true;
                    if unreadable {
                        std::fs::remove_file(&database).unwrap();
                    }
                    let _store = SqliteStore::open_ephemeral(&database).unwrap();
                    let connection = rusqlite::Connection::open(&database).unwrap();
                    connection.execute(
                        "INSERT INTO processes(id,trace_id,started_at,completed_at) VALUES(?1,?1,1,2)",
                        [&process],
                    ).unwrap();
                    SystemTime::UNIX_EPOCH
                },
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OutcomeUnknown);
            assert!(error.message.contains("ended without a result"));
        }
    }
}
