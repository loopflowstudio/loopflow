//! W2-287 — top-level `lf task review` must pass its own authority check.
//!
//! The human review commands refuse to run inside a managed agent session, so a
//! Task cannot answer the review it is the subject of. That boundary is named by
//! the session id vars alone. `with_runtime` mints `LF_RUN_ID`/`LF_PROCESS_ID`
//! for **every** top-level `lf` command before the command body runs, so a guard
//! keyed on them refuses the human CLI it exists to serve.
//!
//! These tests pin both directions: a top-level invocation reaches the review
//! lookup, and a managed session is still refused.

mod support;

use loopflow::interaction_review::InteractionReviewId;
use loopflow::ops::task::{task_review_complete, task_review_message};
use loopflow::store::{open_store, StorageConfig};
use support::EnvGuard;
use tempfile::TempDir;

const AUTHORITY_REFUSAL: &str = "cannot run inside a Task, Project, or Wave agent session";

/// A guarded home with a **materialized** registry, plus the runtime identity
/// `with_runtime` exports before dispatching any command.
///
/// The registry must exist: against an empty home every review command fails at
/// store resolution ("no Loopflow registry on this machine"), which is not the
/// authority error either — so the top-level proof would pass without ever
/// reaching the guard's far side.
fn top_level_cli(home: &TempDir) -> EnvGuard {
    let guard = EnvGuard::with_lf_home(&[], home.path());
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime
        .block_on(open_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        )))
        .expect("materialize the review registry");
    std::env::set_var("LF_RUN_ID", "run_w2287");
    std::env::set_var("LF_PROCESS_ID", "process_w2287");
    guard
}

#[test]
fn top_level_review_commands_pass_authority_and_reach_the_review_lookup() {
    let home = TempDir::new().expect("temp lf home");
    let _guard = top_level_cli(&home);

    // A well-formed id for a review that does not exist. Reaching "not found"
    // means the call cleared the authority boundary and queried the registry —
    // the far side of the guard is the whole subject here.
    let absent = InteractionReviewId::new();

    let message = task_review_message(absent.as_str(), "ship it".to_string())
        .expect_err("no such review exists");
    assert!(
        message.to_string().contains("not found"),
        "a top-level `lf task review message` must reach the review lookup, not \
         be refused by its own authority guard: {message}"
    );

    let complete = task_review_complete(absent.as_str(), "approved", "looks right".to_string())
        .expect_err("no such review exists");
    assert!(
        !complete.to_string().contains(AUTHORITY_REFUSAL),
        "a top-level `lf task review complete` must not be refused by its own \
         authority guard: {complete}"
    );
}

#[test]
fn a_managed_task_session_is_still_refused() {
    let home = TempDir::new().expect("temp lf home");
    // EnvGuard clears the ambient Task identity and restores it on drop, so the
    // session id set here is the only one in play.
    let _guard = top_level_cli(&home);
    std::env::set_var("LF_TASK_SESSION_ID", "ts_w2287");

    let absent = InteractionReviewId::new();

    let message = task_review_message(absent.as_str(), "approve my own review".to_string())
        .expect_err("a Task body cannot answer its own review");
    assert!(
        message.to_string().contains(AUTHORITY_REFUSAL),
        "a Task session must be refused before the review is read: {message}"
    );

    let complete = task_review_complete(absent.as_str(), "approved", "self-approval".to_string())
        .expect_err("a Task body cannot complete its own review");
    assert!(
        complete.to_string().contains(AUTHORITY_REFUSAL),
        "a Task session must be refused before the review is read: {complete}"
    );
}
