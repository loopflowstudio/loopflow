//! Per-Conversation observer receipts. Only the viewer process reports its own
//! attachment; native create and display replies never establish readiness.
//! Its OS lock proves it still runs, so a record cannot outlive a host restart.
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use super::super::{ConversationReadiness, ObserverState};
use super::identity;

/// How long opening waits for observers it launched. A slower attachment is
/// reported as uncertain and settles in the terminal.
const READINESS_WAIT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum Recorded {
    /// Written before the native request; a lost reply stays here and fences retries.
    Pending,
    Launched,
    Connected,
    History,
    Failed {
        reason: String,
    },
    Uncertain {
        reason: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct Record {
    session: String,
    #[serde(flatten)]
    recorded: Recorded,
}

/// One observer as the opening command sees it.
#[derive(Debug)]
pub(super) struct Observer {
    pub session: String,
    recorded: Recorded,
    running: bool,
}

impl Observer {
    /// Whether a new observer may join it. A pending record may yet be running
    /// after a lost reply, and saved history has nothing to connect to.
    pub fn retryable(&self) -> bool {
        !self.running && !matches!(self.recorded, Recorded::Pending | Recorded::History)
    }

    /// The viewer reported; waiting longer cannot change what opening learns.
    fn settled(&self) -> bool {
        !matches!(self.recorded, Recorded::Pending | Recorded::Launched)
    }

    /// The receipt entry, offering `retry` when a retry applies.
    pub fn readiness(&self, retry: &[String]) -> ConversationReadiness {
        use ObserverState::{Connected, Ended, Failed, History, Uncertain};
        let (state, reason) = match &self.recorded {
            Recorded::Pending => (
                Uncertain,
                Some("Observer launch outcome is unknown; inspect the destination"),
            ),
            Recorded::Launched if self.running => (Uncertain, Some("Observer is attaching")),
            Recorded::Launched => (Uncertain, Some("Observer has not reported an attachment")),
            Recorded::Connected if self.running => (Connected, None),
            Recorded::Connected => (
                Ended,
                Some("Observer is no longer running; reopening does not restart it"),
            ),
            Recorded::History => (
                History,
                Some("Saved history shown; there is no live execution to observe"),
            ),
            Recorded::Failed { reason } => (Failed, Some(reason.as_str())),
            Recorded::Uncertain { reason } => (Uncertain, Some(reason.as_str())),
        };
        ConversationReadiness {
            session: self.session.clone(),
            state,
            reason: reason.map(str::to_owned),
            retry_argv: if self.retryable() {
                retry.to_vec()
            } else {
                Vec::new()
            },
        }
    }
}

fn directory(key: &str) -> PathBuf {
    crate::store::lf_home_dir()
        .join("open")
        .join("observers")
        .join(key)
}

fn path(key: &str, session: &str) -> PathBuf {
    directory(key).join(identity(session))
}

fn read_record(file: &mut File) -> Option<Record> {
    let mut text = String::new();
    file.rewind().ok()?;
    file.read_to_string(&mut text).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_record(file: &mut File, record: &Record) -> Result<()> {
    // Never empty in between: a reader identifies the Session from this record.
    let bytes = serde_json::to_vec(record)?;
    file.rewind()?;
    file.write_all(&bytes)?;
    file.set_len(bytes.len() as u64)?;
    Ok(())
}

fn open_record(path: &Path) -> Result<File> {
    Ok(std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?)
}

/// Record the observer about to be launched, unless one already runs there.
pub(super) fn expect(key: &str, session: &str) -> Result<()> {
    std::fs::create_dir_all(directory(key))?;
    let mut file = open_record(&path(key, session))?;
    if file.try_lock_exclusive().is_ok() {
        write_record(
            &mut file,
            &Record {
                session: session.into(),
                recorded: Recorded::Pending,
            },
        )?;
    }
    Ok(())
}

/// The native request was answered. A viewer that already reported keeps its
/// record. A failed write leaves it pending, which only reads as more uncertain.
pub(super) fn confirm(key: &str) {
    let Ok(entries) = std::fs::read_dir(directory(key)) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(mut file) = open_record(&entry.path()) else {
            continue;
        };
        if file.try_lock_exclusive().is_err() {
            continue;
        }
        if let Some(mut record) = read_record(&mut file) {
            if record.recorded == Recorded::Pending {
                record.recorded = Recorded::Launched;
                if let Err(error) = write_record(&mut file, &record) {
                    tracing::debug!(%error, "observer launch was not confirmed");
                }
            }
        }
    }
}

/// A destination created from the template owes nothing to its predecessor.
pub(super) fn clear(key: &str) -> Result<()> {
    match std::fs::remove_dir_all(directory(key)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Every Conversation this destination launched an observer for. An unreadable
/// record is a reporting gap and never changes the native result.
pub(super) fn read(key: &str) -> Vec<Observer> {
    let Ok(entries) = std::fs::read_dir(directory(key)) else {
        return Vec::new();
    };
    let mut observers = Vec::new();
    for entry in entries.flatten() {
        let Ok(mut file) = File::open(entry.path()) else {
            continue;
        };
        let running = file.try_lock_shared().is_err();
        let Some(record) = read_record(&mut file) else {
            continue;
        };
        observers.push(Observer {
            session: record.session,
            recorded: record.recorded,
            running,
        });
    }
    observers.sort_by(|a, b| a.session.cmp(&b.session));
    observers
}

/// Give observers launched by this opening time to report, with one shared deadline.
pub(super) fn settle(keys: &[String]) {
    let deadline = Instant::now() + READINESS_WAIT;
    while Instant::now() < deadline {
        if keys
            .iter()
            .all(|key| read(key).iter().all(Observer::settled))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A running observer's own report. Holding it holds the liveness lock.
#[derive(Debug)]
pub(crate) struct ObserverReport {
    file: File,
    session: String,
}

impl ObserverReport {
    /// A second pane observing the same Session leaves the first one's report alone.
    pub(crate) fn begin(key: &str, session: &str) -> Option<Self> {
        let file = open_record(&path(key, session)).ok()?;
        // An opening command may be reading this record; it holds the lock briefly.
        for _ in 0..10 {
            if file.try_lock_exclusive().is_ok() {
                return Some(Self {
                    file,
                    session: session.into(),
                });
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    // A failed write is a recording gap: the report reads uncertain, the viewer stays.
    fn record(&mut self, recorded: Recorded) {
        let record = Record {
            session: self.session.clone(),
            recorded,
        };
        if let Err(error) = write_record(&mut self.file, &record) {
            tracing::debug!(%error, "observer report was not written");
        }
    }

    pub(crate) fn connected(&mut self) {
        self.record(Recorded::Connected);
    }

    pub(crate) fn history(&mut self) {
        self.record(Recorded::History);
    }

    pub(crate) fn failed(&mut self, error: &anyhow::Error) {
        let reason = format!("{error:#}");
        self.record(if error.is::<crate::transport::OutcomeUnknown>() {
            Recorded::Uncertain { reason }
        } else {
            Recorded::Failed { reason }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Observer, ObserverState, Recorded};

    fn observer(recorded: Recorded, running: bool) -> Observer {
        Observer {
            session: "session".into(),
            recorded,
            running,
        }
    }

    fn state(recorded: Recorded, running: bool) -> ObserverState {
        observer(recorded, running).readiness(&[]).state
    }

    #[test]
    fn only_a_running_acknowledged_observer_is_connected() {
        assert_eq!(state(Recorded::Connected, true), ObserverState::Connected);
        // Host restart and leaving look alike: the process holding the lock is gone.
        assert_eq!(state(Recorded::Connected, false), ObserverState::Ended);
        assert!(observer(Recorded::Connected, false).retryable());
        for recorded in [Recorded::Pending, Recorded::Launched] {
            assert_eq!(state(recorded, false), ObserverState::Uncertain);
        }
    }

    #[test]
    fn a_lost_launch_reply_fences_retries_until_the_observer_reports() {
        assert!(!observer(Recorded::Pending, false).retryable());
        assert!(observer(Recorded::Launched, false).retryable());
        assert!(!observer(Recorded::Launched, true).retryable());
    }

    #[test]
    fn saved_history_is_neither_connected_nor_a_failure_to_retry() {
        let history = observer(Recorded::History, false);
        assert_eq!(history.readiness(&[]).state, ObserverState::History);
        assert!(!history.retryable() && history.settled());
    }

    #[test]
    fn an_unanswered_attachment_stays_uncertain_and_offers_the_retry() {
        let reason = "outcome is unknown".to_owned();
        let retry = ["lf".to_owned()];
        let readiness = observer(
            Recorded::Uncertain {
                reason: reason.clone(),
            },
            false,
        )
        .readiness(&retry);
        assert_eq!(readiness.state, ObserverState::Uncertain);
        assert_eq!(readiness.reason, Some(reason));
        assert_eq!(readiness.retry_argv, retry);
    }
}
