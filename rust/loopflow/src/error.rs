use thiserror::Error;

use serde::Serialize;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("skill or graph not found: {0}. Run `lf list skill` or `lf list graph` to discover definitions, or `lf : <message>` to send a message")]
    TargetNotFound(String),
    #[error("graph not found: {0}. For a skill, use `lf skill {0}`")]
    GraphNotFound(String),
    #[error("skill not found: {0}")]
    SkillNotFound(String),
    #[error("invalid graph: {0}")]
    InvalidGraph(String),
    #[error("invalid skill: {0}")]
    InvalidSkill(String),
    #[error("io error: {0}")]
    Io(String),
}

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("graph not found: {0}")]
    GraphNotFound(String),
    #[error("skill not found: {0}")]
    SkillNotFound(String),
    #[error("invalid graph: {0}")]
    InvalidGraph(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("io error: {0}")]
    IoError(String),
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GitError {
    #[error("{command}: {stderr}")]
    CommandFailed { command: String, stderr: String },
    #[error("io error: {0}")]
    Io(String),
}

impl From<LoadError> for CoreError {
    fn from(err: LoadError) -> Self {
        match err {
            LoadError::GraphNotFound(name) => CoreError::GraphNotFound(name),
            LoadError::SkillNotFound(name) => CoreError::SkillNotFound(name),
            LoadError::InvalidGraph(msg) => CoreError::InvalidGraph(msg),
            other => CoreError::ExecutionFailed(other.to_string()),
        }
    }
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        CoreError::IoError(err.to_string())
    }
}

impl From<GitError> for CoreError {
    fn from(err: GitError) -> Self {
        CoreError::ExecutionFailed(err.to_string())
    }
}

impl From<std::io::Error> for GitError {
    fn from(err: std::io::Error) -> Self {
        GitError::Io(err.to_string())
    }
}

impl From<std::io::Error> for LoadError {
    fn from(err: std::io::Error) -> Self {
        LoadError::Io(err.to_string())
    }
}
