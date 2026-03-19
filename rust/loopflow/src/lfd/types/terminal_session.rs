use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum TerminalSessionStatus {
    Pending,
    Attached,
    Running,
    Succeeded,
    Failed,
    Canceled,
}

impl TerminalSessionStatus {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: LfdId,
    pub wave_id: LfdId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_run_id: Option<LfdId>,
    pub step: String,
    pub agent: String,
    pub cwd: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    pub source: String,
    pub status: TerminalSessionStatus,
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

impl TerminalSession {
    pub fn attach(&mut self) -> bool {
        if self.status != TerminalSessionStatus::Pending {
            return false;
        }
        self.status = TerminalSessionStatus::Attached;
        self.attached_at = Some(OffsetDateTime::now_utc());
        true
    }

    pub fn start(&mut self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        if self.attached_at.is_none() {
            self.attached_at = Some(OffsetDateTime::now_utc());
        }
        self.status = TerminalSessionStatus::Running;
        self.started_at = Some(OffsetDateTime::now_utc());
        true
    }

    pub fn complete(&mut self, exit_code: i32) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.status = TerminalSessionStatus::from_exit_code(exit_code);
        self.completed_at = Some(OffsetDateTime::now_utc());
        true
    }

    pub fn cancel(&mut self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.status = TerminalSessionStatus::Canceled;
        self.completed_at = Some(OffsetDateTime::now_utc());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{TerminalSession, TerminalSessionStatus};
    use crate::lfd::id::LfdId;
    use time::OffsetDateTime;

    fn session(status: TerminalSessionStatus) -> TerminalSession {
        TerminalSession {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            wave_run_id: None,
            step: "design".to_string(),
            agent: "claude".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
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
        let mut session = session(TerminalSessionStatus::Pending);

        assert!(session.attach());
        assert_eq!(session.status, TerminalSessionStatus::Attached);
        assert!(session.attached_at.is_some());
    }

    #[test]
    fn start_auto_attaches_session() {
        let mut session = session(TerminalSessionStatus::Pending);

        assert!(session.start());
        assert_eq!(session.status, TerminalSessionStatus::Running);
        assert!(session.attached_at.is_some());
        assert!(session.started_at.is_some());
    }

    #[test]
    fn terminal_sessions_do_not_restart_or_complete_twice() {
        let mut session = session(TerminalSessionStatus::Succeeded);

        assert!(!session.start());
        assert!(!session.complete(1));
        assert!(!session.cancel());
    }
}
