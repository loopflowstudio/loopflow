//! What a served write answers: its result or error, the store revisions it
//! advanced, and, under a request ID, the first answer again for a repeat.
//! A client that lost a reply sends the write again with the same request ID;
//! the Machine gives it the first answer and runs nothing.

use std::collections::HashSet;
use std::sync::{Condvar, LazyLock, Mutex};

use loopflow_execution::engine::{Error, ErrorCode};
use loopflow_execution::store::sqlite::SqliteStore;
use loopflow_storage::sqlite::command_answers::CommandAnswer;
use loopflow_storage::sqlite::Revisions;

/// One request's answer as the server sends it.
#[derive(Debug, Clone)]
pub(super) struct Answer {
    pub result: Result<serde_json::Value, Error>,
    /// Null for reads and server operations, and for a failed write that
    /// advanced nothing.
    pub revisions: Option<Revisions>,
}

impl Answer {
    pub fn read(result: Result<serde_json::Value, Error>) -> Self {
        Self {
            result,
            revisions: None,
        }
    }
}

/// A write named by its client: the request ID and the operation it names.
#[derive(Debug)]
pub(super) struct Remembered {
    pub request_id: String,
    pub fingerprint: String,
}

impl Remembered {
    pub fn new(request_id: String, operation: &impl serde::Serialize) -> Self {
        use sha2::{Digest, Sha256};
        let operation = serde_json::to_vec(operation).expect("an operation serializes");
        Self {
            request_id,
            fingerprint: format!("{:x}", Sha256::digest(operation)),
        }
    }
}

/// Request IDs whose write is running, so a repeat waits for the first answer.
static RUNNING: LazyLock<(Mutex<HashSet<String>>, Condvar)> = LazyLock::new(Default::default);

struct Running(String);

impl Running {
    fn claim(request_id: &str) -> Self {
        let (running, finished) = &*RUNNING;
        let mut running = running.lock().expect("running request IDs");
        while running.contains(request_id) {
            running = finished.wait(running).expect("running request IDs");
        }
        running.insert(request_id.to_owned());
        Self(request_id.to_owned())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let (running, finished) = &*RUNNING;
        running.lock().expect("running request IDs").remove(&self.0);
        finished.notify_all();
    }
}

/// Run a write on this blocking worker and answer it with the revisions it
/// advanced. Under a request ID, a remembered answer is returned instead and
/// `run` never starts; a new answer is remembered unless its outcome is unknown.
pub(super) fn write(
    remembered: Option<Remembered>,
    run: impl FnOnce() -> Result<serde_json::Value, Error>,
) -> Answer {
    let _running = remembered
        .as_ref()
        .map(|remembered| Running::claim(&remembered.request_id));
    let store = open();
    if let (Some(remembered), Some(store)) = (&remembered, &store) {
        match store.command_answer(&remembered.request_id) {
            Ok(Some(stored)) if stored.fingerprint == remembered.fingerprint => {
                return Answer {
                    result: stored.outcome.map_err(|error| {
                        serde_json::from_value(error).expect("a remembered error decodes")
                    }),
                    revisions: stored.revisions,
                };
            }
            Ok(Some(_)) => {
                return Answer::read(Err(Error::new(
                    ErrorCode::Invalid,
                    "this request ID already names a different change",
                )
                .with(serde_json::json!({ "request_id": remembered.request_id }))))
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "remembered answers are unreadable; running the write")
            }
        }
    }
    let before = store.as_ref().and_then(|store| store.revision_map().ok());
    let result = run();
    // A write can create the store it ran against.
    let store = store.or_else(open);
    let after = store.as_ref().and_then(|store| store.revision_map().ok());
    let advanced = after.map(|after| after.advanced_since(&before.unwrap_or_default()));
    let answer = Answer {
        revisions: match &result {
            Ok(_) => advanced,
            Err(_) => advanced.filter(|advanced| !advanced.0.is_empty()),
        },
        result,
    };
    let unknown = matches!(&answer.result, Err(error) if error.code == ErrorCode::OutcomeUnknown);
    if let (Some(remembered), Some(store), false) = (remembered, &store, unknown) {
        let stored = CommandAnswer {
            fingerprint: remembered.fingerprint,
            outcome: answer
                .result
                .clone()
                .map_err(|error| serde_json::to_value(error).expect("an error serializes")),
            revisions: answer.revisions.clone(),
        };
        if let Err(error) = store.record_command_answer(&remembered.request_id, &stored) {
            tracing::warn!(request_id = %remembered.request_id, %error, "answer was not remembered; a repeat runs again");
        }
    }
    answer
}

fn open() -> Option<SqliteStore> {
    let path = loopflow_execution::store::default_db_path();
    if !path.exists() {
        return None;
    }
    loopflow_execution::store::open_sqlite(&path)
        .inspect_err(|error| tracing::warn!(%error, "store unavailable to answer a write"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::{write, Remembered};
    use loopflow_execution::engine::ErrorCode;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn store() -> (
        tempfile::TempDir,
        loopflow_execution::config::environment::EnvVarGuard,
    ) {
        let home = tempfile::tempdir().unwrap();
        let guard = loopflow_execution::config::environment::EnvVarGuard::set(
            "LF_HOME",
            home.path().to_str().unwrap(),
        );
        loopflow_execution::store::open_sqlite(&loopflow_execution::store::default_db_path())
            .unwrap();
        (home, guard)
    }

    #[test]
    fn a_repeated_request_id_returns_the_first_answer_without_running_again() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let _home = store();
        let runs = AtomicUsize::new(0);
        let run = || {
            runs.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({ "run": runs.load(Ordering::SeqCst) }))
        };
        let operation = serde_json::json!({"kind": "task.complete", "issue": "LOO-1"});
        let first = write(Some(Remembered::new("r".into(), &operation)), run);
        let again = write(Some(Remembered::new("r".into(), &operation)), run);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert_eq!(first.result.unwrap(), again.result.unwrap());
        assert_eq!(first.revisions, again.revisions);
        // A write without a request ID always runs.
        write(None, run);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_different_change_under_a_request_id_is_invalid() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let _home = store();
        let complete = serde_json::json!({"kind": "task.complete", "issue": "LOO-1"});
        let abandon = serde_json::json!({"kind": "task.abandon", "issue": "LOO-1"});
        write(Some(Remembered::new("r".into(), &complete)), || {
            Ok(serde_json::Value::Null)
        });
        let refused = write(Some(Remembered::new("r".into(), &abandon)), || {
            panic!("a mismatched repeat never runs")
        });
        assert_eq!(refused.result.unwrap_err().code, ErrorCode::Invalid);
    }

    #[test]
    fn a_refusal_that_changed_nothing_carries_no_revisions() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let _home = store();
        let refused = write(None, || {
            Err(loopflow_execution::engine::Error::new(
                ErrorCode::Conflict,
                "its PR #12 is open",
            ))
        });
        assert!(refused.revisions.is_none());
        let changed_nothing = write(None, || Ok(serde_json::Value::Null));
        assert_eq!(changed_nothing.revisions.unwrap().0.len(), 0);
    }
}
