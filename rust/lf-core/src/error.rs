use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("run not found: {0}")]
    RunNotFound(String),
    #[error("step run not found: {0}")]
    StepRunNotFound(String),
    #[error("store error: {0}")]
    Other(String),
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("flow not found: {0}")]
    FlowNotFound(String),
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("direction not found: {0}")]
    DirectionNotFound(String),
    #[error("invalid flow: {0}")]
    InvalidFlow(String),
    #[error("invalid step: {0}")]
    InvalidStep(String),
    #[error("invalid direction: {0}")]
    InvalidDirection(String),
    #[error("io error: {0}")]
    Io(String),
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("flow not found: {0}")]
    FlowNotFound(String),
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("invalid flow: {0}")]
    InvalidFlow(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("worktree error: {0}")]
    WorktreeError(String),
    #[error("store error: {0}")]
    StoreError(String),
    #[error("io error: {0}")]
    IoError(String),
}

impl From<StoreError> for CoreError {
    fn from(err: StoreError) -> Self {
        CoreError::StoreError(err.to_string())
    }
}

impl From<LoadError> for CoreError {
    fn from(err: LoadError) -> Self {
        match err {
            LoadError::FlowNotFound(name) => CoreError::FlowNotFound(name),
            LoadError::StepNotFound(name) => CoreError::StepNotFound(name),
            LoadError::InvalidFlow(msg) => CoreError::InvalidFlow(msg),
            other => CoreError::ExecutionFailed(other.to_string()),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::IoError(err.to_string())
    }
}

impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        LoadError::Io(err.to_string())
    }
}
