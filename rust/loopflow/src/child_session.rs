//! Durable control shared by Project and Task Sessions.
//!
//! This module owns typed child identity, control attribution, process
//! generations, commands, directives, decisions, and observation envelopes.
//! Project and Task keep their own lifecycle states, events, runners, and
//! public CLI nouns; there is no generic child lifecycle.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::TaskSessionId;

macro_rules! prefixed_uuid_id {
    ($name:ident, $prefix:literal, $error:ty, $invalid:path) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}{}", $prefix, uuid::Uuid::new_v4().simple()))
            }

            pub fn parse(value: &str) -> Result<Self, $error> {
                let suffix = value
                    .strip_prefix($prefix)
                    .ok_or_else(|| $invalid(format!("expected {} id", $prefix)))?;
                uuid::Uuid::parse_str(suffix).map_err(|error| $invalid(error.to_string()))?;
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

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }
    };
}

pub(crate) use prefixed_uuid_id;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildSessionDataError {
    #[error("invalid child-session id: {0}")]
    InvalidId(String),
}

prefixed_uuid_id!(
    ChildCommandId,
    "cc_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);
prefixed_uuid_id!(
    ChildDecisionId,
    "cd_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);
prefixed_uuid_id!(
    ChildDirectiveId,
    "dir_",
    ChildSessionDataError,
    ChildSessionDataError::InvalidId
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObservationRecipient {
    Wave { wave_id: WaveId },
    Project { session_id: ProjectSessionId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Durable receipt for one child-process generation. The latest receipt stays
/// on a Session after the process exits so recovery can reject stale runners
/// and advance the generation monotonically.
pub struct ChildProcessGeneration {
    pub generation: u32,
    pub pid: Option<u32>,
    pub tmux_name: String,
    pub started_at: OffsetDateTime,
}

/// The executable and store a Session runs against, pinned once when the
/// Session is created.
///
/// Re-deriving these per process is how a Session gets relaunched by a
/// worktree's `target/debug/lf` against the live registry: the launching
/// process's own binary and environment decided the child's, so whoever
/// happened to type the command chose the child's execution context. Pinning
/// makes the Session the authority, and every relaunch reproduces the context
/// the Session was born with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildExecutionContext {
    pub lf_bin: PathBuf,
    pub db_path: PathBuf,
    pub lf_home: PathBuf,
}

#[cfg(test)]
impl ChildExecutionContext {
    /// A pinned context for tests that do not care which binary or store a
    /// Session names — only that it names one.
    pub(crate) fn for_tests() -> Self {
        Self {
            lf_bin: PathBuf::from("/usr/local/bin/lf"),
            db_path: PathBuf::from("/tmp/loopflow-test/loopflow.db"),
            lf_home: PathBuf::from("/tmp/loopflow-test"),
        }
    }
}

/// A recorded request to end a Session, durable from the moment the Abandon
/// command is queued rather than from the moment a runner consumes it.
///
/// The gap between those two moments is where a supervisor used to restart a
/// Session someone had already ended: the pending command carried the intent,
/// but the Session row still read `Running`, and every launch path reads the
/// Session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbandonIntent {
    pub requested_at: OffsetDateTime,
    pub reason: String,
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
    /// Provider delivery has begun. A process crash from here is ambiguous.
    Delivering,
    Accepted,
    Failed,
    Superseded,
    /// The process died after delivery began but before Loopflow recorded the outcome.
    Uncertain,
}

impl ChildCommandState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Persisted => "persisted",
            Self::Claimed => "claimed",
            Self::Delivering => "delivering",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
            Self::Uncertain => "uncertain",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Failed | Self::Superseded | Self::Uncertain
        )
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
    Wave(WaveId),
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
