//! Turn vocabulary: `ChatTurn`, the wire type Loopflow consumes.
//!
//! The wave's loop runs each turn as a bounded `wave` child inside
//! the RESIDENT process (see [`crate::flowloop::wave`]) and reports it as
//! resident wire deltas ([`crate::wave::wire`]), folded by the listener's
//! runtime into journaled, broadcast turns.
//!
//! Mapping:
//! - a human message becomes one `user` turn;
//! - each agent turn (text + tool activity, closed by the vendor's turn
//!   completion) becomes one `assistant` turn whose `items` capture the
//!   commands/edits/messages it ran.

use serde::{Deserialize, Serialize};

use crate::chat::types::{ConversationItem, Lifecycle};
use crate::project_session::{ProjectEventKind, ProjectObservation};
use crate::task::{TaskEventKind, TaskObservation};
use crate::wave::playhead::{now_rfc3339, BodyProvenance};

/// Who authored a turn. Mirrors Swift `MessageRole` (user/assistant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildActivitySubject {
    Project,
    Task,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildActivityKind {
    StateChanged,
    ControlApplied,
    Directed,
    Incorporated,
    DecisionRequired,
    DecisionResolved,
    PullRequestOpened,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildControlActivity {
    pub id: String,
    pub subject: ChildActivitySubject,
    pub subject_id: String,
    pub session_id: String,
    pub kind: ChildActivityKind,
    pub title: String,
    pub summary: String,
    pub directive_version: Option<u32>,
    pub command_id: Option<String>,
    pub effect: Option<String>,
    pub decision_id: Option<String>,
    pub options: Vec<String>,
}

impl ChildControlActivity {
    pub fn from_task(observation: &TaskObservation) -> Self {
        let (kind, title, summary, directive_version, command_id, effect, decision_id, options) =
            task_activity_fields(&observation.event);
        Self {
            id: observation.inbox_id(),
            subject: ChildActivitySubject::Task,
            subject_id: observation.issue_identifier.clone(),
            session_id: observation.session_id.to_string(),
            kind,
            title,
            summary,
            directive_version,
            command_id,
            effect,
            decision_id,
            options,
        }
    }

    pub fn from_project(observation: &ProjectObservation) -> Self {
        let (kind, title, summary, directive_version, command_id, effect, decision_id, options) =
            project_activity_fields(&observation.event);
        Self {
            id: observation.inbox_id(),
            subject: ChildActivitySubject::Project,
            subject_id: observation.project.clone(),
            session_id: observation.session_id.to_string(),
            kind,
            title,
            summary,
            directive_version,
            command_id,
            effect,
            decision_id,
            options,
        }
    }
}

/// One turn in a wave chat — the unit the chat server streams.
///
/// Wire type consumed by Loopflow. Every field is required (no serde defaults):
/// the same shape round-trips through Rust and Swift.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatTurn {
    /// Stable within the wave journal across restarts: `"turn-1"`, `"turn-2"`, …
    pub id: String,
    pub role: ChatRole,
    /// Accumulated assistant prose (or the human message for a `user` turn).
    pub text: String,
    /// Lifecycle of the turn. A `user` turn is always `Completed`.
    pub status: Lifecycle,
    /// Tool/command/file/message items the agent produced, in order.
    pub items: Vec<ConversationItem>,
    /// RFC 3339 timestamp of when the turn opened.
    pub created_at: String,
    /// Speaker label for attributed emissions (`lf radio pub` — worker reports,
    /// child-wave escalations). Absent for the loop's own turns and plain
    /// user turns.
    pub from: Option<String>,
    /// Body that produced an assistant span. Required on the wire and
    /// explicitly null for human/attributed turns.
    pub body: Option<BodyProvenance>,
    /// Structured child motion rendered as a linked activity card. Required on
    /// the wire and explicitly null for ordinary conversation turns.
    pub activity: Option<ChildControlActivity>,
}

impl ChatTurn {
    /// A completed `user` turn carrying a human message.
    pub fn user(id: String, text: String) -> Self {
        Self {
            id,
            role: ChatRole::User,
            text,
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: now_rfc3339(),
            from: None,
            body: None,
            activity: None,
        }
    }

    /// The one turn-growth rule every projection shares: `Message` prose
    /// joins into `text` (newline-separated), every other item appends to
    /// `items`. The listener's open-turn snapshot, the journal fold
    /// (`fold_thread`), and the resident's adapter (`EventAdapter`) all grow
    /// turns through this — a second copy is the live-vs-replay split-brain
    /// the journal exists to kill.
    pub fn absorb_item(&mut self, item: ConversationItem) {
        if let ConversationItem::Message { text, phase, .. } = &item {
            if phase.as_deref() == Some("stream") {
                self.text.push_str(text);
            } else {
                self.push_text(text);
            }
        } else {
            self.items.push(item);
        }
    }

    /// Close the body that produced this turn, if it had one. Every terminal
    /// path goes through here — the live finalizers, the boot janitor, and the
    /// journal fold — so a replayed turn's body reads exactly as the live one
    /// did. Human and attributed turns have no body and close as a no-op.
    pub fn close_body(&mut self, ended_at: String, reason: Option<String>) {
        if let Some(body) = self.body.as_mut() {
            body.ended_at = Some(ended_at);
            body.termination_reason = reason;
        }
    }

    /// Join a prose fragment into the turn text, newline-separated.
    pub fn push_text(&mut self, fragment: &str) {
        if !self.text.is_empty() {
            self.text.push('\n');
        }
        self.text.push_str(fragment);
    }
}

type ActivityFields = (
    ChildActivityKind,
    String,
    String,
    Option<u32>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
);

fn task_activity_fields(event: &TaskEventKind) -> ActivityFields {
    match event {
        TaskEventKind::Started => activity(ChildActivityKind::StateChanged, "Task started", ""),
        TaskEventKind::StatusChanged { to, reason, .. } => activity(
            ChildActivityKind::StateChanged,
            &format!("Task is {}", to.as_str()),
            reason,
        ),
        TaskEventKind::CommandChanged {
            command_id,
            state,
            effect,
            error,
        } => (
            ChildActivityKind::ControlApplied,
            format!("Control {}", state.as_str()),
            error.clone().unwrap_or_default(),
            None,
            Some(command_id.to_string()),
            effect.map(|value| value.as_str().to_string()),
            None,
            Vec::new(),
        ),
        TaskEventKind::DirectiveChanged { version, .. } => (
            ChildActivityKind::Directed,
            format!("Direction v{version}"),
            "Waiting for incorporation".to_string(),
            Some(*version),
            None,
            None,
            None,
            Vec::new(),
        ),
        TaskEventKind::DirectiveIncorporated {
            version, summary, ..
        } => (
            ChildActivityKind::Incorporated,
            format!("Incorporated direction v{version}"),
            summary.clone(),
            Some(*version),
            None,
            None,
            None,
            Vec::new(),
        ),
        TaskEventKind::DecisionRequested {
            decision_id,
            prompt,
            options,
        } => (
            ChildActivityKind::DecisionRequired,
            "Decision required".to_string(),
            prompt.clone(),
            None,
            None,
            None,
            Some(decision_id.to_string()),
            options.clone(),
        ),
        TaskEventKind::DecisionResolved { choice, .. } => activity(
            ChildActivityKind::DecisionResolved,
            "Decision resolved",
            choice,
        ),
        TaskEventKind::Progress { summary } => {
            activity(ChildActivityKind::StateChanged, "Task progress", summary)
        }
        TaskEventKind::PullRequestOpened { number, url } => activity(
            ChildActivityKind::PullRequestOpened,
            &format!("Opened PR #{number}"),
            url,
        ),
        TaskEventKind::Completed { summary, .. } => {
            activity(ChildActivityKind::Completed, "Task completed", summary)
        }
        TaskEventKind::Failed { error, .. } => {
            activity(ChildActivityKind::Failed, "Task failed", error)
        }
    }
}

fn project_activity_fields(event: &ProjectEventKind) -> ActivityFields {
    match event {
        ProjectEventKind::Started => {
            activity(ChildActivityKind::StateChanged, "Project started", "")
        }
        ProjectEventKind::StatusChanged { to, reason, .. } => activity(
            ChildActivityKind::StateChanged,
            &format!("Project is {}", to.as_str()),
            reason,
        ),
        ProjectEventKind::CommandChanged {
            command_id,
            state,
            effect,
            error,
        } => (
            ChildActivityKind::ControlApplied,
            format!("Control {}", state.as_str()),
            error.clone().unwrap_or_default(),
            None,
            Some(command_id.to_string()),
            effect.map(|value| value.as_str().to_string()),
            None,
            Vec::new(),
        ),
        ProjectEventKind::DirectiveChanged { version, .. } => (
            ChildActivityKind::Directed,
            format!("Direction v{version}"),
            "Waiting for incorporation".to_string(),
            Some(*version),
            None,
            None,
            None,
            Vec::new(),
        ),
        ProjectEventKind::DirectiveIncorporated {
            version, summary, ..
        } => (
            ChildActivityKind::Incorporated,
            format!("Incorporated direction v{version}"),
            summary.clone(),
            Some(*version),
            None,
            None,
            None,
            Vec::new(),
        ),
        ProjectEventKind::TaskObserved { event, .. } => task_activity_fields(event),
        ProjectEventKind::DecisionRequested {
            decision_id,
            prompt,
            options,
        } => (
            ChildActivityKind::DecisionRequired,
            "Decision required".to_string(),
            prompt.clone(),
            None,
            None,
            None,
            Some(decision_id.to_string()),
            options.clone(),
        ),
        ProjectEventKind::DecisionResolved { choice, .. } => activity(
            ChildActivityKind::DecisionResolved,
            "Decision resolved",
            choice,
        ),
        ProjectEventKind::IterationCompleted { summary, .. } => activity(
            ChildActivityKind::StateChanged,
            "Project iteration completed",
            summary,
        ),
        ProjectEventKind::Completed { summary } => {
            activity(ChildActivityKind::Completed, "Project completed", summary)
        }
        ProjectEventKind::Failed { error, .. } => {
            activity(ChildActivityKind::Failed, "Project failed", error)
        }
    }
}

fn activity(kind: ChildActivityKind, title: &str, summary: &str) -> ActivityFields {
    (
        kind,
        title.to_string(),
        summary.to_string(),
        None,
        None,
        None,
        None,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_turn_round_trips_through_json() {
        let turn = ChatTurn::user("turn-0".into(), "please fix the build".into());
        let value = serde_json::to_value(&turn).expect("serialize");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, turn);
        assert_eq!(decoded.role, ChatRole::User);
    }

    #[test]
    fn attributed_turn_round_trips_and_absent_from_decodes_none() {
        let mut turn = ChatTurn::user("turn-1".into(), "worker report".into());
        turn.from = Some("worker".to_string());
        let value = serde_json::to_value(&turn).expect("serialize");
        assert_eq!(value["from"], "worker");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded.from.as_deref(), Some("worker"));

        // Absent `from` is None — no default masking on the wire.
        let mut value =
            serde_json::to_value(ChatTurn::user("turn-2".into(), "hi".into())).expect("serialize");
        value.as_object_mut().expect("object").remove("from");
        let decoded: ChatTurn = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded.from, None);
    }

    #[test]
    fn absorb_item_joins_prose_and_appends_the_rest() {
        let mut turn = ChatTurn::user("turn-3".into(), String::new());
        turn.absorb_item(ConversationItem::Message {
            id: "m-1".into(),
            text: "first".into(),
            phase: None,
        });
        turn.absorb_item(ConversationItem::Tool {
            id: "t-1".into(),
            name: "Bash".into(),
            status: Lifecycle::Completed,
            input: None,
            output: None,
        });
        turn.absorb_item(ConversationItem::Message {
            id: "m-2".into(),
            text: "second".into(),
            phase: None,
        });

        assert_eq!(turn.text, "first\nsecond");
        assert_eq!(turn.items.len(), 1, "prose joins text, tools append");
    }

    #[test]
    fn absorb_item_concatenates_stream_fragments_exactly() {
        let mut turn = ChatTurn::user("turn-4".into(), String::new());
        for text in ["hello", " ", "world"] {
            turn.absorb_item(ConversationItem::Message {
                id: format!("m-{}", turn.text.len()),
                text: text.into(),
                phase: Some("stream".into()),
            });
        }

        assert_eq!(turn.text, "hello world");
    }

    #[test]
    fn decision_activity_keeps_options_and_lineage() {
        let decision_id = crate::child_session::ChildDecisionId::new();
        let observation = TaskObservation {
            session_id: crate::task::TaskSessionId::new(),
            issue_identifier: "INF-123".to_string(),
            event_id: 9,
            event: TaskEventKind::DecisionRequested {
                decision_id: decision_id.clone(),
                prompt: "Which parser mode?".to_string(),
                options: vec!["strict".to_string(), "permissive".to_string()],
            },
        };

        let activity = ChildControlActivity::from_task(&observation);
        assert_eq!(activity.kind, ChildActivityKind::DecisionRequired);
        assert_eq!(activity.decision_id.as_deref(), Some(decision_id.as_str()));
        assert_eq!(activity.options, ["strict", "permissive"]);
    }
}
