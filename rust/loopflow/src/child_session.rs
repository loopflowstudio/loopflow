use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::lfd::id::LfdId;
use crate::project_session::ProjectSessionId;
use crate::task::TaskSessionId;

macro_rules! string_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, ChildSessionDataError> {
                let suffix = value.strip_prefix($prefix).ok_or_else(|| {
                    ChildSessionDataError::InvalidId(format!("expected {} id", $prefix))
                })?;
                Uuid::parse_str(suffix)
                    .map_err(|error| ChildSessionDataError::InvalidId(error.to_string()))?;
                Ok(Self::from_raw(value.to_string()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = ChildSessionDataError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildSessionDataError {
    #[error("invalid child-session id: {0}")]
    InvalidId(String),
}

string_id!(ChildCommandId, "cc_");
string_id!(ChildDecisionId, "cd_");
string_id!(ChildDirectiveId, "dir_");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionSupervisor {
    Wave { wave_id: LfdId },
    Project { session_id: ProjectSessionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildProcess {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChildCommandKind {
    FollowUp {
        text: String,
    },
    Steer {
        text: String,
    },
    Interrupt {
        replacement: Option<String>,
    },
    Resume {
        message: Option<String>,
    },
    Decide {
        decision_id: ChildDecisionId,
        choice: String,
        message: Option<String>,
    },
    Abandon {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildCommandState {
    Persisted,
    Claimed,
    Accepted,
    Failed,
    Superseded,
}

impl ChildCommandState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Persisted => "persisted",
            Self::Claimed => "claimed",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Accepted | Self::Failed | Self::Superseded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChildCommandEffect {
    LiveSteer,
    NextTurn,
    Replacement,
    Decision,
}

impl ChildCommandEffect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiveSteer => "live_steer",
            Self::NextTurn => "next_turn",
            Self::Replacement => "replacement",
            Self::Decision => "decision",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildCommandSource {
    Wave(LfdId),
    Project(ProjectSessionId),
    Human,
    Attachment,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ChildRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

impl ChildRef {
    pub fn target_kind(&self) -> &'static str {
        match self {
            Self::Project(_) => "project",
            Self::Task(_) => "task",
        }
    }

    pub fn target_id(&self) -> &str {
        match self {
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

pub(crate) fn unincorporated_directive_version(
    current_version: u32,
    incorporated_version: u32,
) -> Option<u32> {
    (current_version > incorporated_version).then_some(current_version)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DirectiveKind {
    Initial,
    Replacement,
    WorkRevised,
}

impl DirectiveKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Replacement => "replacement",
            Self::WorkRevised => "work_revised",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildDirective {
    pub id: ChildDirectiveId,
    pub target: ChildRef,
    pub version: u32,
    pub kind: DirectiveKind,
    pub text: String,
    pub source: ChildCommandSource,
    pub command_id: Option<ChildCommandId>,
    pub issued_at: OffsetDateTime,
    pub applied_at: Option<OffsetDateTime>,
    pub incorporated_at: Option<OffsetDateTime>,
    pub incorporated_summary: Option<String>,
}

impl ChildDirective {
    pub fn initial(target: ChildRef, text: String, source: ChildCommandSource) -> Self {
        Self {
            id: ChildDirectiveId::new(),
            target,
            version: 1,
            kind: DirectiveKind::Initial,
            text,
            source,
            command_id: None,
            issued_at: OffsetDateTime::now_utc(),
            applied_at: None,
            incorporated_at: None,
            incorporated_summary: None,
        }
    }

    pub fn replacement(
        target: ChildRef,
        version: u32,
        text: String,
        source: ChildCommandSource,
        command_id: ChildCommandId,
    ) -> Self {
        Self {
            id: ChildDirectiveId::new(),
            target,
            version,
            kind: DirectiveKind::Replacement,
            text,
            source,
            command_id: Some(command_id),
            issued_at: OffsetDateTime::now_utc(),
            applied_at: None,
            incorporated_at: None,
            incorporated_summary: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildCommand {
    pub id: ChildCommandId,
    pub target: ChildRef,
    pub source: ChildCommandSource,
    pub kind: ChildCommandKind,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub created_at: OffsetDateTime,
    pub claimed_by_generation: Option<u32>,
    pub accepted_at: Option<OffsetDateTime>,
    pub error: Option<String>,
}

impl ChildCommand {
    pub fn new(target: ChildRef, source: ChildCommandSource, kind: ChildCommandKind) -> Self {
        let effect = match &kind {
            ChildCommandKind::FollowUp { .. } | ChildCommandKind::Resume { message: Some(_) } => {
                Some(ChildCommandEffect::NextTurn)
            }
            ChildCommandKind::Interrupt {
                replacement: Some(_),
            } => Some(ChildCommandEffect::Replacement),
            ChildCommandKind::Decide { .. } => Some(ChildCommandEffect::Decision),
            ChildCommandKind::Steer { .. }
            | ChildCommandKind::Interrupt { replacement: None }
            | ChildCommandKind::Resume { message: None }
            | ChildCommandKind::Abandon { .. } => None,
        };
        Self {
            id: ChildCommandId::new(),
            target,
            source,
            kind,
            state: ChildCommandState::Persisted,
            effect,
            created_at: OffsetDateTime::now_utc(),
            claimed_by_generation: None,
            accepted_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryResult<S> {
    Commands(Vec<ChildCommand>),
    Stopped(S),
}

#[cfg(test)]
mod tests {
    use super::{
        unincorporated_directive_version, ChildCommandId, ChildDecisionId, ChildDirectiveId,
    };

    #[test]
    fn child_ids_are_prefixed_and_round_trip() {
        let command = ChildCommandId::new();
        let decision = ChildDecisionId::new();
        let directive = ChildDirectiveId::new();

        assert_eq!(ChildCommandId::parse(command.as_str()).unwrap(), command);
        assert_eq!(ChildDecisionId::parse(decision.as_str()).unwrap(), decision);
        assert_eq!(
            ChildDirectiveId::parse(directive.as_str()).unwrap(),
            directive
        );
    }

    #[test]
    fn a_newer_directive_blocks_the_flow_boundary_until_incorporated() {
        assert_eq!(unincorporated_directive_version(2, 1), Some(2));
        assert_eq!(unincorporated_directive_version(2, 2), None);
    }
}
