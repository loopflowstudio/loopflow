//! Errors from saved data and atomic storage commands.

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Selection(#[from] crate::work::selection::SelectionError),
    #[error(transparent)]
    Planning(#[from] crate::planning::LinearBindingError),
    #[error(transparent)]
    TaskData(#[from] crate::work::task::TaskDataError),
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Wave {0} is no longer available in this repository")]
    WaveUnavailable(loopflow_model::id::WaveId),
    #[error("not found")]
    NotFound,
    /// A compare-and-swap named a revision that is no longer current.
    #[error("{0}")]
    Stale(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("competing planning {effect} receipts")]
    PlanningReceiptConflict { effect: &'static str },
    #[error("invalid control authority: {0}")]
    InvalidAuthority(String),
}

pub type StoreResult<T> = Result<T, StoreError>;
