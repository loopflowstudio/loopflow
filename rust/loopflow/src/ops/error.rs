use thiserror::Error;

use crate::error::{CoreError, GitError, LoadError};

pub type OpsResult<T> = Result<T, OpsError>;

#[derive(Debug, Error)]
pub enum OpsError {
    #[error(transparent)]
    Planning(#[from] crate::linear::LinearBindingError),
    #[error(transparent)]
    TaskData(#[from] crate::work::task::TaskDataError),
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
    #[error("{0}")]
    DeliveryHeld(String),
    #[error("checkout is busy: {0}")]
    CheckoutBusy(String),
    #[error("release deferred: {reason}; continuation: {continuation}")]
    ReleaseDeferred {
        reason: String,
        continuation: String,
    },
    /// A person dequeued the release PR or disabled its auto-merge after this
    /// run armed it.
    #[error(
        "release PR #{pr_number} was {} by {} at {}; the release stops. Run `lf release run` to resume.",
        if *.queued { "removed from the merge queue" } else { "taken off auto-merge" },
        .actor.as_deref().unwrap_or("someone"),
        .at.to_rfc3339()
    )]
    ManualMergeRemoval {
        pr_number: u64,
        actor: Option<String>,
        at: chrono::DateTime<chrono::Utc>,
        queued: bool,
    },
    #[error("sync onto {onto} failed ({detail})")]
    SyncConflict {
        onto: String,
        detail: String,
        recovery: Option<Box<crate::ops::sync::SyncRecovery>>,
    },
    #[error(
        "refusing to sync: recorded base {base} is not an ancestor of HEAD, so \
         the saved comparison cannot identify the child edits. Keep this branch; \
         review its history before changing the stack comparison base"
    )]
    UnsafeSyncBase { base: String },
}
