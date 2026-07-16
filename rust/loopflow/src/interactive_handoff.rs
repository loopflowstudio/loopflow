//! Durable evidence for deliberately interactive provider work.
//!
//! A handoff is a child Session beneath one Wave, Project Session, or Task
//! Session. It is the durable waiting marker and presentation contract; it does
//! not own a provider process. The parent's existing body generation remains
//! authoritative for process ownership and recovery.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::child_session::prefixed_uuid_id;
use crate::engine::wave_home::{HomeLocation, WaveHome};
use crate::id::WaveId;
use crate::project_session::ProjectSessionId;
use crate::task::TaskSessionId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InteractiveHandoffDataError {
    #[error("invalid interactive-handoff id: {0}")]
    InvalidId(String),
    #[error("invalid interactive-handoff parent: {0}")]
    InvalidParent(String),
    #[error("invalid interactive-handoff status: {0}")]
    InvalidStatus(String),
    #[error("invalid interactive handoff: {0}")]
    InvalidInvariant(String),
}

prefixed_uuid_id!(
    InteractiveHandoffId,
    "ih_",
    InteractiveHandoffDataError,
    InteractiveHandoffDataError::InvalidId
);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum InteractiveHandoffParent {
    Wave(WaveId),
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

impl InteractiveHandoffParent {
    pub fn parse(value: &str) -> Result<Self, InteractiveHandoffDataError> {
        let (kind, id) = value.split_once(':').ok_or_else(|| {
            InteractiveHandoffDataError::InvalidParent(
                "expected wave:<id>, project:<id>, or task:<id>".to_string(),
            )
        })?;
        match kind {
            "wave" => WaveId::parse(id)
                .map(Self::Wave)
                .map_err(|error| InteractiveHandoffDataError::InvalidParent(error.to_string())),
            "project" => ProjectSessionId::parse(id)
                .map(Self::Project)
                .map_err(|error| InteractiveHandoffDataError::InvalidParent(error.to_string())),
            "task" => TaskSessionId::parse(id)
                .map(Self::Task)
                .map_err(|error| InteractiveHandoffDataError::InvalidParent(error.to_string())),
            _ => Err(InteractiveHandoffDataError::InvalidParent(format!(
                "unknown parent kind {kind:?}"
            ))),
        }
    }

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

impl std::fmt::Display for InteractiveHandoffParent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.kind(), self.id())
    }
}

impl FromStr for InteractiveHandoffParent {
    type Err = InteractiveHandoffDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractiveHandoffStatus {
    Waiting,
    Attached,
    Completed,
    HandedBack,
    Failed,
}

impl InteractiveHandoffStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Attached => "attached",
            Self::Completed => "completed",
            Self::HandedBack => "handed_back",
            Self::Failed => "failed",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::HandedBack | Self::Failed)
    }
}

impl FromStr for InteractiveHandoffStatus {
    type Err = InteractiveHandoffDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "waiting" => Ok(Self::Waiting),
            "attached" => Ok(Self::Attached),
            "completed" => Ok(Self::Completed),
            "handed_back" => Ok(Self::HandedBack),
            "failed" => Ok(Self::Failed),
            value => Err(InteractiveHandoffDataError::InvalidStatus(
                value.to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum InteractiveHandoffOutcome {
    Completed { summary: String },
    HandedBack { summary: String },
    Failed { reason: String },
}

impl InteractiveHandoffOutcome {
    pub fn status(&self) -> InteractiveHandoffStatus {
        match self {
            Self::Completed { .. } => InteractiveHandoffStatus::Completed,
            Self::HandedBack { .. } => InteractiveHandoffStatus::HandedBack,
            Self::Failed { .. } => InteractiveHandoffStatus::Failed,
        }
    }

    pub fn validate(&self) -> Result<(), InteractiveHandoffDataError> {
        let text = match self {
            Self::Completed { summary } | Self::HandedBack { summary } => summary,
            Self::Failed { reason } => reason,
        };
        if text.trim().is_empty() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "terminal outcome text cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenInteractiveHandoff {
    pub parent: InteractiveHandoffParent,
    pub home: WaveHome,
    pub cwd: PathBuf,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub body_generation: u32,
    pub reason: String,
    pub environment: BTreeMap<String, String>,
    pub attach_argv: Vec<String>,
}

impl OpenInteractiveHandoff {
    pub fn validate(&self) -> Result<(), InteractiveHandoffDataError> {
        if self.provider.trim().is_empty() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "provider cannot be empty".to_string(),
            ));
        }
        if self
            .provider_session_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "provider session id cannot be empty".to_string(),
            ));
        }
        if self.body_generation == 0 {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "body generation must be positive".to_string(),
            ));
        }
        if self.reason.trim().is_empty() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "reason cannot be empty".to_string(),
            ));
        }
        if !self.cwd.is_absolute() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "cwd must be absolute".to_string(),
            ));
        }
        if self.attach_argv.is_empty()
            || self.attach_argv.iter().any(|argument| argument.is_empty())
        {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "attach argv must contain only non-empty arguments".to_string(),
            ));
        }
        if self.environment.keys().any(|key| key.trim().is_empty()) {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "environment keys cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveHandoff {
    pub id: InteractiveHandoffId,
    pub parent: InteractiveHandoffParent,
    pub wave_id: WaveId,
    pub home: WaveHome,
    pub cwd: PathBuf,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub body_generation: u32,
    pub reason: String,
    pub environment: BTreeMap<String, String>,
    pub attach_argv: Vec<String>,
    pub status: InteractiveHandoffStatus,
    pub outcome: Option<InteractiveHandoffOutcome>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub attached_at: Option<OffsetDateTime>,
    pub terminal_at: Option<OffsetDateTime>,
    pub wake_claimed_at: Option<OffsetDateTime>,
    pub wake_claimed_by_generation: Option<u32>,
}

impl InteractiveHandoff {
    pub fn validate(&self) -> Result<(), InteractiveHandoffDataError> {
        OpenInteractiveHandoff {
            parent: self.parent.clone(),
            home: self.home.clone(),
            cwd: self.cwd.clone(),
            provider: self.provider.clone(),
            provider_session_id: self.provider_session_id.clone(),
            body_generation: self.body_generation,
            reason: self.reason.clone(),
            environment: self.environment.clone(),
            attach_argv: self.attach_argv.clone(),
        }
        .validate()?;
        if self.parent.kind() == "wave" && self.parent.id() != self.wave_id.as_str() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "Wave parent must equal owning Wave id".to_string(),
            ));
        }
        let terminal = self.status.is_terminal();
        if terminal != self.terminal_at.is_some() || terminal != self.outcome.is_some() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "terminal status, timestamp, and outcome must agree".to_string(),
            ));
        }
        if let Some(outcome) = &self.outcome {
            outcome.validate()?;
            if outcome.status() != self.status {
                return Err(InteractiveHandoffDataError::InvalidInvariant(
                    "terminal outcome must match status".to_string(),
                ));
            }
        }
        if self.attached_at.is_some() && self.status == InteractiveHandoffStatus::Waiting {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "waiting handoff cannot have attach evidence".to_string(),
            ));
        }
        if self.wake_claimed_at.is_some() != self.wake_claimed_by_generation.is_some() {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "wake claim timestamp and generation must agree".to_string(),
            ));
        }
        if self.wake_claimed_at.is_some() && !terminal {
            return Err(InteractiveHandoffDataError::InvalidInvariant(
                "only a terminal handoff can have a wake claim".to_string(),
            ));
        }
        Ok(())
    }

    pub fn attach_descriptor(&self) -> InteractiveHandoffAttach {
        let host = match self.home.location() {
            HomeLocation::Local => "localhost".to_string(),
            HomeLocation::Remote { .. } => self
                .home
                .to_string()
                .strip_prefix("ssh://")
                .and_then(|address| address.split_once('@').map(|(_, host)| host))
                .unwrap_or("localhost")
                .to_string(),
        };
        InteractiveHandoffAttach {
            session_id: self.id.clone(),
            status: self.status,
            cwd: self.cwd.clone(),
            host,
            environment: self.environment.clone(),
            argv: self.attach_argv.clone(),
        }
    }

    /// A handoff still expecting a human: its parent is blocked on it. Terminal
    /// outcomes (completed, handed back, failed) are no longer active.
    pub fn is_active(&self) -> bool {
        !self.status.is_terminal()
    }

    /// The census/display row for `lf handoff list`. `now` computes `age_secs`
    /// against `updated_at`; timestamps that will not format leave the string
    /// empty and the age `None` rather than fabricating a value.
    pub fn list_row(&self, now: OffsetDateTime) -> InteractiveHandoffListRow {
        let updated_at = format_rfc3339(self.updated_at);
        let age_secs =
            (!updated_at.is_empty()).then(|| (now - self.updated_at).whole_seconds().max(0));
        InteractiveHandoffListRow {
            session_id: self.id.clone(),
            parent_kind: self.parent.kind().to_string(),
            parent_id: self.parent.id().to_string(),
            wave_id: self.wave_id.clone(),
            status: self.status,
            provider: self.provider.clone(),
            provider_session_id: self.provider_session_id.clone(),
            home: self.home.to_string(),
            cwd: self.cwd.clone(),
            reason: self.reason.clone(),
            created_at: format_rfc3339(self.created_at),
            updated_at,
            age_secs,
        }
    }
}

fn format_rfc3339(ts: OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

/// One row of `lf handoff list --json`: enough to census and display a durable
/// handoff without a second read, but never its attach argv or environment.
/// Presentations that decide to Open re-fetch the attach descriptor through
/// `lf handoff attach`, which is the contract's first-attach evidence point.
/// `parent_kind` is `wave`/`project`/`task`; `home` is the canonical Home
/// address. `age_secs` is `now - updated_at`, `None` when the timestamp cannot
/// be read — an unknown age is never a zero one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveHandoffListRow {
    pub session_id: InteractiveHandoffId,
    pub parent_kind: String,
    pub parent_id: String,
    pub wave_id: WaveId,
    pub status: InteractiveHandoffStatus,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub home: String,
    pub cwd: PathBuf,
    pub reason: String,
    pub created_at: String,
    pub updated_at: String,
    pub age_secs: Option<i64>,
}

/// Stable wire contract consumed by terminal and app presentation adapters.
/// Terminal bytes stay behind `argv`; they never enter this DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveHandoffAttach {
    pub session_id: InteractiveHandoffId,
    pub status: InteractiveHandoffStatus,
    pub cwd: PathBuf,
    pub host: String,
    pub environment: BTreeMap<String, String>,
    pub argv: Vec<String>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::{
        InteractiveHandoffAttach, InteractiveHandoffParent, InteractiveHandoffStatus,
        OpenInteractiveHandoff,
    };
    use crate::engine::wave_home::WaveHome;
    use crate::task::TaskSessionId;

    #[test]
    fn parent_reference_round_trips_without_a_generic_session_noun() {
        let parent = InteractiveHandoffParent::Task(TaskSessionId::new());
        assert_eq!(
            InteractiveHandoffParent::parse(&parent.to_string()).unwrap(),
            parent
        );
    }

    #[test]
    fn open_contract_rejects_relative_cwd_and_empty_attach_argv() {
        let mut open = OpenInteractiveHandoff {
            parent: InteractiveHandoffParent::Task(TaskSessionId::new()),
            home: WaveHome::parse("jack@local").unwrap(),
            cwd: PathBuf::from("repo"),
            provider: "codex".to_string(),
            provider_session_id: None,
            body_generation: 1,
            reason: "Needs an OAuth login".to_string(),
            environment: BTreeMap::new(),
            attach_argv: vec!["tmux".to_string()],
        };
        assert!(open.validate().is_err());
        open.cwd = PathBuf::from("/repo");
        open.attach_argv.clear();
        assert!(open.validate().is_err());
    }

    #[test]
    fn terminal_statuses_are_explicit() {
        assert!(!InteractiveHandoffStatus::Waiting.is_terminal());
        assert!(!InteractiveHandoffStatus::Attached.is_terminal());
        assert!(InteractiveHandoffStatus::HandedBack.is_terminal());
    }

    #[test]
    fn interactive_handoff_attach_fixture_round_trips() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/interactive_handoff_attach.json"
        ));
        let attach: InteractiveHandoffAttach = serde_json::from_str(fixture).unwrap();
        assert_eq!(attach.status, InteractiveHandoffStatus::Attached);
        assert_eq!(attach.host, "localhost");
        assert_eq!(attach.argv[0], "tmux");
        let encoded = serde_json::to_string(&attach).unwrap();
        assert_eq!(
            serde_json::from_str::<InteractiveHandoffAttach>(&encoded).unwrap(),
            attach
        );
    }

    #[test]
    fn interactive_handoff_list_fixture_round_trips() {
        use super::InteractiveHandoffListRow;
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/interactive_handoff_list.json"
        ));
        let rows: Vec<InteractiveHandoffListRow> = serde_json::from_str(fixture).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].parent_kind, "task");
        assert_eq!(rows[0].status, InteractiveHandoffStatus::Waiting);
        assert_eq!(rows[0].age_secs, Some(90));
        // An unreadable timestamp keeps age None, not a fabricated zero.
        assert_eq!(rows[1].parent_kind, "project");
        assert_eq!(rows[1].provider_session_id, None);
        assert_eq!(rows[1].age_secs, None);
        let encoded = serde_json::to_string(&rows).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<InteractiveHandoffListRow>>(&encoded).unwrap(),
            rows
        );
    }
}
