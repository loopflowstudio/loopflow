//! Durable input and execution identity shared by Wave, Project, and Task Work.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

macro_rules! durable_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, DurableDataError> {
                let suffix = value.strip_prefix($prefix).ok_or_else(|| {
                    DurableDataError::InvalidId(format!("expected {} id", $prefix))
                })?;
                uuid::Uuid::parse_str(suffix)
                    .map_err(|error| DurableDataError::InvalidId(error.to_string()))?;
                Ok(Self(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = DurableDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DurableDataError {
    #[error("invalid durable id: {0}")]
    InvalidId(String),
    #[error("invalid epoch state: {0}")]
    InvalidEpochState(String),
    #[error("invalid send state: {0}")]
    InvalidSendState(String),
}

durable_id!(ProjectId, "proj_");
durable_id!(TaskId, "task_");
durable_id!(EpochId, "epoch_");
durable_id!(RunId, "run_");
durable_id!(SteerId, "steer_");
durable_id!(SendId, "send_");
durable_id!(DecisionId, "decision_");
durable_id!(ApprovalId, "approval_");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum WorkRef {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl WorkRef {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave",
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpochState {
    Open,
    Done,
    Abandoned,
}

impl EpochState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Done => "done",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "open" => Ok(Self::Open),
            "done" => Ok(Self::Done),
            "abandoned" => Ok(Self::Abandoned),
            value => Err(DurableDataError::InvalidEpochState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Basis {
    pub epoch_id: EpochId,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Epoch {
    pub id: EpochId,
    pub work: WorkRef,
    pub number: u32,
    pub state: EpochState,
    pub current_basis: Basis,
    pub created_at: OffsetDateTime,
    pub terminal_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Author {
    User,
    Run(RunId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    pub id: SteerId,
    pub work: WorkRef,
    pub basis: Basis,
    pub author: Author,
    pub text: String,
    pub issued_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendVia {
    Live,
    Seed,
}

impl SendVia {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Seed => "seed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendState {
    Sending,
    Sent,
    Failed,
    Unknown,
}

impl SendState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, DurableDataError> {
        match value {
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            "unknown" => Ok(Self::Unknown),
            value => Err(DurableDataError::InvalidSendState(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Send {
    pub id: SendId,
    pub steer_id: SteerId,
    pub turn_id: String,
    pub via: SendVia,
    pub state: SendState,
    pub provider_turn_id: Option<String>,
    pub reason: Option<String>,
    pub attempted_at: OffsetDateTime,
    pub finished_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteerReceipt {
    pub steer: Steer,
    pub sends: Vec<Send>,
    pub applied_by: Option<Basis>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundarySeed {
    pub basis: Basis,
    pub steers: Vec<Steer>,
}

impl BoundarySeed {
    pub fn render(&self) -> String {
        if self.steers.is_empty() {
            return String::new();
        }
        let direction = self
            .steers
            .iter()
            .map(|steer| format!("- [rev {}] {}", steer.basis.revision, steer.text))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "<lf:steers basis=\"{}:{}\">\n{}\n</lf:steers>",
            self.basis.epoch_id, self.basis.revision, direction
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionWrite {
    pub request_id: String,
    pub choice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionReceipt {
    pub id: DecisionId,
    pub work: WorkRef,
    pub basis: Basis,
    pub request_id: String,
    pub choice: String,
    pub decided_at: OffsetDateTime,
}

/// Authenticated Home-local entrypoint. It is deliberately not serializable.
#[derive(Debug)]
pub struct AuthenticatedRequest {
    _private: (),
}

impl AuthenticatedRequest {
    pub(crate) fn cli() -> Self {
        Self { _private: () }
    }
}

/// An already-validated Run capability. Tokens never enter durable DTOs.
#[derive(Debug, Clone)]
pub struct RunLease {
    pub run_id: RunId,
    pub work: WorkRef,
    pub basis: Basis,
}

#[derive(Debug)]
pub enum ControlCtx<'a> {
    User(&'a AuthenticatedRequest),
    Run(&'a RunLease),
}

#[cfg(test)]
mod tests {
    use super::{Basis, BoundarySeed, EpochId, Steer};
    use crate::durable::{Author, ProjectId, SteerId, WorkRef};

    #[test]
    fn ordered_steers_render_as_one_basis_projection() {
        let epoch_id = EpochId::new();
        let work = WorkRef::Project(ProjectId::new());
        let steer = |revision, text: &str| Steer {
            id: SteerId::new(),
            work: work.clone(),
            basis: Basis {
                epoch_id: epoch_id.clone(),
                revision,
            },
            author: Author::User,
            text: text.to_string(),
            issued_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        let seed = BoundarySeed {
            basis: Basis {
                epoch_id: epoch_id.clone(),
                revision: 3,
            },
            steers: vec![steer(2, "first"), steer(3, "second")],
        };

        assert_eq!(
            seed.render(),
            format!(
                "<lf:steers basis=\"{}:3\">\n- [rev 2] first\n- [rev 3] second\n</lf:steers>",
                seed.basis.epoch_id
            )
        );
    }
}
