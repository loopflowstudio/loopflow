use thiserror::Error;

use crate::engine::error::{CoreError, GitError, LoadError};

pub type OpsResult<T> = Result<T, OpsError>;

#[derive(Debug, Error)]
pub enum OpsError {
    #[error("git error: {0}")]
    Git(#[from] GitError),
    #[error("core error: {0}")]
    Core(#[from] CoreError),
    #[error("load error: {0}")]
    Load(#[from] LoadError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("command failed: {command}\n{stderr}")]
    CommandFailed { command: String, stderr: String },
    #[error("parse error: {0}")]
    Parse(String),
    #[error("{0}")]
    Message(String),
    #[error("checkout is busy: {0}")]
    CheckoutBusy(String),
    #[error("release deferred: {reason}; continuation: {continuation}")]
    ReleaseDeferred {
        reason: String,
        continuation: String,
    },
    #[error("Task {issue} is {state} and cannot be completed")]
    TaskCompletionConflict { issue: String, state: String },
    #[error("sync onto {onto} failed ({detail})")]
    SyncConflict {
        onto: String,
        detail: String,
        recovery: Option<Box<crate::ops::sync::SyncRecovery>>,
    },
    #[error(
        "refusing to sync: recorded base {base} is not an ancestor of HEAD, so \
         its history diverged. Reconcile by hand; the commits since the common \
         ancestor are:\n{commits}"
    )]
    UnsafeSyncBase { base: String, commits: String },
}
