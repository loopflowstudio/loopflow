use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

pub const TMUX_TERMINAL_SOURCE: &str = "wave_skill_tmux";
pub const PALETTE_TERMINAL_SOURCE: &str = "palette";
/// A bare `lf` invocation that self-registered from inside a wave context.
/// Not tmux-backed: the process lives inside an ancestor's terminal, so
/// attach/cancel must not manage tmux for it; the process marks itself
/// terminal via the completion-token path.
pub const LF_CLI_SOURCE: &str = "lf_cli";
/// A self-registered `lf loop` server: the process is owned by the wave
/// server, not launched by lfd. The session row records the running server
/// (endpoint + pid in `env`) so Loopflow sees the loop and one-brain
/// enforcement has a fact to key on.
pub const WAVE_SERVER_SOURCE: &str = "wave_server";

/// Env key on a `wave_server` session carrying the server's loopback
/// `host:port` (the `.wave-endpoint` address).
pub const WAVE_SERVER_ENDPOINT_ENV: &str = "LF_WAVE_ENDPOINT";
/// Env key on a `wave_server` session carrying the server's pid, used by
/// session reconciliation to detect a crashed server.
pub const WAVE_SERVER_PID_ENV: &str = "LF_WAVE_SERVER_PID";

/// Build a human-readable tmux session name from the branch name.
///
/// Tmux session names cannot contain dots or colons, so those are replaced
/// with hyphens.
pub fn tmux_session_name(branch: &str) -> String {
    format!("lf-{}", branch.replace(['.', ':'], "-"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Pending,
    Attached,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

pub const LIVE_SESSION_STATUSES: &[SessionStatus] = &[
    SessionStatus::Pending,
    SessionStatus::Attached,
    SessionStatus::Running,
];

impl SessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Attached => "attached",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    pub(crate) fn as_i32(self) -> i32 {
        match self {
            Self::Pending => 0,
            Self::Attached => 1,
            Self::Running => 2,
            Self::Succeeded => 3,
            Self::Failed => 4,
            Self::Canceled => 5,
        }
    }

    pub(crate) fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Pending,
            1 => Self::Attached,
            2 => Self::Running,
            3 => Self::Succeeded,
            4 => Self::Failed,
            5 => Self::Canceled,
            _ => Self::Failed,
        }
    }

    pub fn from_exit_code(exit_code: i32) -> Self {
        if exit_code == 0 {
            Self::Succeeded
        } else {
            Self::Failed
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum SessionUse {
    WaveAgent,
    Worker,
    Palette,
}

impl SessionUse {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WaveAgent => "wave_agent",
            Self::Worker => "worker",
            Self::Palette => "palette",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseSessionUseError {
    value: String,
}

impl fmt::Display for ParseSessionUseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown session use '{}'", self.value)
    }
}

impl std::error::Error for ParseSessionUseError {}

impl TryFrom<&str> for SessionUse {
    type Error = ParseSessionUseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "wave_agent" => Ok(Self::WaveAgent),
            "worker" => Ok(Self::Worker),
            "palette" => Ok(Self::Palette),
            _ => Err(ParseSessionUseError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: LfdId,
    pub wave_id: LfdId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<LfdId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<LfdId>,
    #[serde(rename = "use")]
    pub session_use: SessionUse,
    pub skill: String,
    pub agent: String,
    pub cwd: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    pub source: String,
    /// Human-readable tmux session name (e.g. `lf-engbot-6aec3220`).
    /// Set at creation, used for attach and cleanup.
    #[serde(default)]
    pub tmux_name: String,
    pub status: SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    #[serde(skip_serializing)]
    pub completion_token: Option<String>,
}

impl Session {
    pub fn is_tmux_backed(&self) -> bool {
        matches!(
            self.source.as_str(),
            TMUX_TERMINAL_SOURCE | PALETTE_TERMINAL_SOURCE
        )
    }

    pub fn attach(&mut self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        let mut changed = false;
        if self.status == SessionStatus::Pending {
            self.status = SessionStatus::Attached;
            changed = true;
        }
        if self.attached_at.is_none() {
            self.attached_at = Some(OffsetDateTime::now_utc());
            changed = true;
        }
        changed
    }

    pub fn start(&mut self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        if self.attached_at.is_none() {
            self.attached_at = Some(OffsetDateTime::now_utc());
        }
        self.status = SessionStatus::Running;
        self.started_at = Some(OffsetDateTime::now_utc());
        true
    }

    pub fn complete(&mut self, exit_code: i32) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.status = SessionStatus::from_exit_code(exit_code);
        self.completed_at = Some(OffsetDateTime::now_utc());
        true
    }

    pub fn cancel(&mut self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.status = SessionStatus::Canceled;
        self.completed_at = Some(OffsetDateTime::now_utc());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Session, SessionStatus, SessionUse};
    use crate::lfd::id::LfdId;
    use time::OffsetDateTime;

    fn session(status: SessionStatus) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Palette,
            skill: "design".to_string(),
            agent: "claude".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: "wave_skill".to_string(),
            tmux_name: "lf-test-branch".to_string(),
            status,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[test]
    fn attach_marks_pending_sessions_attached() {
        let mut session = session(SessionStatus::Pending);

        assert!(session.attach());
        assert_eq!(session.status, SessionStatus::Attached);
        assert!(session.attached_at.is_some());
    }

    #[test]
    fn start_auto_attaches_session() {
        let mut session = session(SessionStatus::Pending);

        assert!(session.start());
        assert_eq!(session.status, SessionStatus::Running);
        assert!(session.attached_at.is_some());
        assert!(session.started_at.is_some());
    }

    #[test]
    fn sessions_do_not_restart_or_complete_twice() {
        let mut session = session(SessionStatus::Succeeded);

        assert!(!session.start());
        assert!(!session.complete(1));
        assert!(!session.cancel());
    }

    #[test]
    fn session_use_rejects_unknown_values() {
        let err = SessionUse::try_from("legacy").unwrap_err();

        assert_eq!(err.to_string(), "unknown session use 'legacy'");
    }
}
