use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::{RunId, WorkRef};

pub(crate) const WORK_STARTUP_ATTEMPT_ENV: &str = "LF_WORK_STARTUP_ATTEMPT";
pub(crate) const WORK_STARTUP_RECEIPT_ENV: &str = "LF_WORK_STARTUP_RECEIPT";

const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(crate) enum WorkStartupState {
    Running {
        work: WorkRef,
        run_id: RunId,
        trace_id: String,
        process_id: String,
        pid: u32,
        process_started_at: i64,
    },
    Parked {
        work: WorkRef,
    },
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WorkStartupReceipt {
    pub attempt_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(flatten)]
    pub state: WorkStartupState,
}

/// Hidden `__work` startup attempt shared by the launcher and controller body.
///
/// This type is public only because the `lf` binary is a separate crate from
/// the library that owns controller execution.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct WorkStartupAttempt {
    attempt_id: String,
    receipt_path: PathBuf,
}

impl WorkStartupAttempt {
    pub(crate) fn new(lf_home: &Path) -> Result<Self> {
        let attempt_id = uuid::Uuid::new_v4().simple().to_string();
        let receipt_path = lf_home
            .join("controller/startup")
            .join(format!("{attempt_id}.json"));
        if let Some(parent) = receipt_path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "cannot create controller startup receipt directory {}",
                    parent.display()
                )
            })?;
        }
        Ok(Self {
            attempt_id,
            receipt_path,
        })
    }

    pub(crate) fn environment(&self) -> [(String, String); 2] {
        [
            (
                WORK_STARTUP_ATTEMPT_ENV.to_string(),
                self.attempt_id.clone(),
            ),
            (
                WORK_STARTUP_RECEIPT_ENV.to_string(),
                self.receipt_path.display().to_string(),
            ),
        ]
    }

    pub(crate) async fn wait(
        self,
        expected: &WorkRef,
        tmux_name: &str,
    ) -> Result<WorkStartupReceipt> {
        self.wait_with_timeout(expected, Some(tmux_name), STARTUP_TIMEOUT)
            .await
    }

    pub(crate) fn settle_failed(&self, reason: impl std::fmt::Display) -> Result<&Path> {
        let state = WorkStartupState::Failed {
            reason: reason.to_string(),
        };
        let persisted = self.write_once(state.clone())?;
        if persisted.state != state {
            return Err(anyhow!(
                "controller startup attempt {} was already settled as {:?}; cannot report transport failure",
                self.attempt_id,
                persisted.state
            ));
        }
        Ok(&self.receipt_path)
    }

    async fn wait_with_timeout(
        self,
        expected: &WorkRef,
        tmux_name: Option<&str>,
        timeout: Duration,
    ) -> Result<WorkStartupReceipt> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match read_receipt(&self.receipt_path) {
                Ok(receipt) => {
                    let receipt = validate_receipt(&self, receipt, expected)?;
                    return Ok(receipt);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(anyhow!(
                        "cannot read controller startup receipt {}: {error}",
                        self.receipt_path.display()
                    ))
                }
            }
            if let Some(tmux_name) = tmux_name {
                let session_exists =
                    match crate::engine::process::tmux_session_exists(tmux_name).await {
                        Ok(session_exists) => session_exists,
                        Err(error) => {
                            self.report_failed(format!(
                            "could not verify controller process during startup for {} {}: {error}",
                            expected.kind(),
                            expected.id()
                        ))?;
                            let receipt = read_receipt(&self.receipt_path)?;
                            return validate_receipt(&self, receipt, expected);
                        }
                    };
                if !session_exists {
                    self.report_failed(format!(
                        "controller process exited before acknowledging startup for {} {}",
                        expected.kind(),
                        expected.id()
                    ))?;
                    let receipt = read_receipt(&self.receipt_path)?;
                    return validate_receipt(&self, receipt, expected);
                }
            }
            if tokio::time::Instant::now() >= deadline {
                self.report_failed(format!(
                    "controller process did not acknowledge startup within 10s for {} {}",
                    expected.kind(),
                    expected.id()
                ))?;
                let receipt = read_receipt(&self.receipt_path)?;
                return validate_receipt(&self, receipt, expected);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

impl WorkStartupAttempt {
    #[doc(hidden)]
    pub fn take_from_env() -> Result<Option<Self>> {
        let attempt_id = std::env::var_os(WORK_STARTUP_ATTEMPT_ENV);
        let receipt_path = std::env::var_os(WORK_STARTUP_RECEIPT_ENV);
        std::env::remove_var(WORK_STARTUP_ATTEMPT_ENV);
        std::env::remove_var(WORK_STARTUP_RECEIPT_ENV);
        match (attempt_id, receipt_path) {
            (None, None) => Ok(None),
            (Some(attempt_id), Some(receipt_path)) => Ok(Some(Self {
                attempt_id: attempt_id.into_string().map_err(|_| {
                    anyhow!("controller startup attempt id is not valid UTF-8")
                })?,
                receipt_path: PathBuf::from(receipt_path),
            })),
            _ => Err(anyhow!(
                "controller startup requires both {WORK_STARTUP_ATTEMPT_ENV} and {WORK_STARTUP_RECEIPT_ENV}"
            )),
        }
    }

    pub(crate) fn report_running(&self, work: WorkRef, run_id: RunId) -> Result<()> {
        let owner = crate::journal::current_exec_process_receipt()
            .context("controller body has no exact live Exec receipt")?;
        self.report_success(WorkStartupState::Running {
            work,
            run_id,
            trace_id: owner.trace_id,
            process_id: owner.exec_id,
            pid: owner.pid,
            process_started_at: owner.started_at,
        })
    }

    pub(crate) fn report_parked(&self, work: WorkRef) -> Result<()> {
        self.report_success(WorkStartupState::Parked { work })
    }

    #[doc(hidden)]
    pub fn report_failed(&self, reason: impl std::fmt::Display) -> Result<()> {
        self.write_once(WorkStartupState::Failed {
            reason: reason.to_string(),
        })?;
        Ok(())
    }

    fn report_success(&self, state: WorkStartupState) -> Result<()> {
        let persisted = self.write_once(state.clone())?;
        if persisted.state != state {
            return Err(anyhow!(
                "controller startup attempt {} was already settled as {:?}; cannot acknowledge {:?}",
                self.attempt_id,
                persisted.state,
                state
            ));
        }
        Ok(())
    }

    fn write_once(&self, state: WorkStartupState) -> Result<WorkStartupReceipt> {
        let receipt = WorkStartupReceipt {
            attempt_id: self.attempt_id.clone(),
            observed_at: OffsetDateTime::now_utc(),
            state,
        };
        write_receipt(&self.receipt_path, &receipt)?;
        let persisted = read_receipt(&self.receipt_path)?;
        if persisted.attempt_id != self.attempt_id {
            return Err(anyhow!(
                "controller startup receipt {} belongs to attempt {}, not {}",
                self.receipt_path.display(),
                persisted.attempt_id,
                self.attempt_id
            ));
        }
        Ok(persisted)
    }
}

fn validate_receipt(
    attempt: &WorkStartupAttempt,
    receipt: WorkStartupReceipt,
    expected: &WorkRef,
) -> Result<WorkStartupReceipt> {
    if receipt.attempt_id != attempt.attempt_id {
        return Err(anyhow!(
            "controller startup receipt {} belongs to attempt {}, not {}",
            attempt.receipt_path.display(),
            receipt.attempt_id,
            attempt.attempt_id
        ));
    }
    match &receipt.state {
        WorkStartupState::Running { work, .. } if work == expected => Ok(receipt),
        WorkStartupState::Running { work, .. } => Err(anyhow!(
            "controller startup receipt {} belongs to {} {}, not {} {}",
            attempt.receipt_path.display(),
            work.kind(),
            work.id(),
            expected.kind(),
            expected.id()
        )),
        WorkStartupState::Parked { work } if work == expected => Ok(receipt),
        WorkStartupState::Parked { work } => Err(anyhow!(
            "controller startup receipt {} belongs to {} {}, not {} {}",
            attempt.receipt_path.display(),
            work.kind(),
            work.id(),
            expected.kind(),
            expected.id()
        )),
        WorkStartupState::Failed { reason } => Err(anyhow!(
            "{} {} controller exited during startup: {reason}; receipt: {}",
            expected.kind(),
            expected.id(),
            attempt.receipt_path.display()
        )),
    }
}

fn read_receipt(path: &Path) -> Result<WorkStartupReceipt, std::io::Error> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}

pub(crate) fn read_work_startup_receipts_at(
    lf_home: &Path,
) -> Result<Vec<WorkStartupReceipt>, std::io::Error> {
    let root = lf_home.join("controller/startup");
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut receipts = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(receipt) = serde_json::from_slice::<WorkStartupReceipt>(&bytes) else {
            continue;
        };
        receipts.push(receipt);
    }
    receipts.sort_by_key(|receipt| receipt.observed_at);
    Ok(receipts)
}

fn write_receipt(path: &Path, receipt: &WorkStartupReceipt) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("controller startup receipt path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec(receipt)?)?;
    file.sync_all()?;
    let result = std::fs::hard_link(&temporary, path);
    let _ = std::fs::remove_file(&temporary);
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::{ProjectId, TaskId};

    #[tokio::test]
    async fn failed_child_receipt_preserves_the_actionable_reason() {
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let signal = attempt.clone();
        let expected = WorkRef::Task(TaskId::new());

        signal
            .report_failed("provider account route is unavailable")
            .unwrap();
        let error = attempt
            .wait_with_timeout(&expected, None, STARTUP_TIMEOUT)
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("provider account route is unavailable"));
        assert!(signal.receipt_path.is_file());
    }

    #[test]
    fn failed_receipt_rejects_a_late_successful_acknowledgment() {
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let signal = attempt;
        let work = WorkRef::Task(TaskId::new());

        signal.report_failed("startup timed out").unwrap();
        let error = signal.report_parked(work).unwrap_err();

        assert!(error.to_string().contains("already settled as Failed"));
        let persisted = read_receipt(&signal.receipt_path).unwrap();
        assert!(matches!(
            persisted.state,
            WorkStartupState::Failed { reason } if reason == "startup timed out"
        ));
    }

    #[tokio::test]
    async fn running_receipt_must_name_the_launched_work() {
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let signal = attempt.clone();
        let expected = WorkRef::Task(TaskId::new());
        let other = WorkRef::Task(TaskId::new());

        signal
            .write_once(WorkStartupState::Running {
                work: other,
                run_id: RunId::new(),
                trace_id: "trace".to_string(),
                process_id: "process".to_string(),
                pid: 42,
                process_started_at: 7,
            })
            .unwrap();
        let error = attempt
            .wait_with_timeout(&expected, None, STARTUP_TIMEOUT)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("not task"));
        assert!(error.to_string().contains(expected.id()));
    }

    #[tokio::test]
    async fn running_receipt_carries_run_and_process_identity() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let repo = tempfile::tempdir().unwrap();
        crate::journal::emit(
            repo.path(),
            crate::journal::LfNode::Run,
            crate::journal::LfEventType::Started,
            crate::journal::LfEventFields {
                command: Some(vec!["lf".to_string(), "__work".to_string()]),
                ..crate::journal::LfEventFields::default()
            },
        );
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let signal = attempt.clone();
        let expected = WorkRef::Project(ProjectId::new());
        let run_id = RunId::new();

        signal
            .report_running(expected.clone(), run_id.clone())
            .unwrap();
        let owner = crate::journal::current_exec_process_receipt().unwrap();
        let receipt = attempt
            .wait_with_timeout(&expected, None, STARTUP_TIMEOUT)
            .await
            .unwrap();
        assert!(matches!(
            receipt.state,
            WorkStartupState::Running {
                work,
                run_id: observed_run,
                trace_id,
                process_id,
                pid,
                process_started_at,
            } if work == expected
                && observed_run == run_id
                && trace_id == owner.trace_id
                && process_id == owner.exec_id
                && pid == owner.pid
                && process_started_at == owner.started_at
        ));
        crate::journal::emit(
            repo.path(),
            crate::journal::LfNode::Run,
            crate::journal::LfEventType::Completed,
            crate::journal::LfEventFields::default(),
        );
        drop(ledger);
    }

    #[tokio::test]
    async fn receipt_attempt_must_match_the_launch() {
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let expected = WorkRef::Task(TaskId::new());
        write_receipt(
            &attempt.receipt_path,
            &WorkStartupReceipt {
                attempt_id: "different-attempt".to_string(),
                observed_at: OffsetDateTime::now_utc(),
                state: WorkStartupState::Failed {
                    reason: "wrong launch".to_string(),
                },
            },
        )
        .unwrap();

        let error = attempt
            .wait_with_timeout(&expected, None, STARTUP_TIMEOUT)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("belongs to attempt"));
        assert!(error.to_string().contains("different-attempt"));
    }

    #[tokio::test]
    async fn parked_receipt_is_a_successful_non_running_boundary() {
        let home = tempfile::tempdir().unwrap();
        let attempt = WorkStartupAttempt::new(home.path()).unwrap();
        let signal = attempt.clone();
        let expected = WorkRef::Task(TaskId::new());

        signal.report_parked(expected.clone()).unwrap();
        let receipt = attempt
            .wait_with_timeout(&expected, None, STARTUP_TIMEOUT)
            .await
            .unwrap();

        assert!(matches!(receipt.state, WorkStartupState::Parked { work } if work == expected));
    }
}
