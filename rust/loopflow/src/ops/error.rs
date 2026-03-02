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
    #[error("agent failed: {0}")]
    AgentFailed(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("{0}")]
    Message(String),
}
