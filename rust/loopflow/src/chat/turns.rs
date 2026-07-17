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
use crate::child_session::{ChildBodyOutcome, ChildProcessGeneration};
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
    PrOpened,
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
}

impl ChildControlActivity {
    pub fn from_task(observation: &TaskObservation) -> Self {
        let fields = task_activity_fields(&observation.event);
        Self {
            id: observation.inbox_id(),
            subject: ChildActivitySubject::Task,
            subject_id: observation.issue_identifier.clone(),
            session_id: observation.session_id.to_string(),
            kind: fields.kind,
            title: fields.title,
            summary: fields.summary,
        }
    }

    pub fn from_project(observation: &ProjectObservation) -> Self {
        let fields = project_activity_fields(&observation.event);
        Self {
            id: observation.inbox_id(),
            subject: ChildActivitySubject::Project,
            subject_id: observation.project.clone(),
            session_id: observation.session_id.to_string(),
            kind: fields.kind,
            title: fields.title,
            summary: fields.summary,
        }
    }
}

/// One turn in a wave chat — the unit the chat server streams.
///
/// Wire type consumed by Loopflow. Every field is required (no serde defaults):
/// the same shape round-trips through Rust and Swift.
#[derive(Debug, Clone, PartialEq, Serialize)]
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

/// One live increment to an open turn: the `turn-delta` SSE frame. The listener
/// broadcasts one per non-finalizing content delta instead of re-serializing the
/// whole `ChatTurn` (see `crate::wave::runtime` — that whole-turn-per-token
/// re-broadcast was O(prose²) on the wire). The client applies it with
/// [`ChatTurn::absorb_item`] against the turn named by `turn_id`, reconstructing
/// exactly what the listener holds; the finalized whole `turn` frame then
/// re-baselines it at turn close. Every field required — same DTO discipline as
/// [`ChatTurn`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnDelta {
    /// The open turn this increment grows (`"turn-<n>"`).
    pub turn_id: String,
    /// The item to absorb — a stream `Message` fragment grows `text`, any other
    /// item appends to `items`, exactly as the listener's fold does.
    pub item: ConversationItem,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatTurnError {
    #[error("child activity entries cannot also carry prose, items, or a provider body")]
    MixedActivity,
    #[error("child activity entries must be completed attributed user-side entries")]
    InvalidActivityEnvelope,
    #[error("human turns must be completed and cannot carry provider items or a body")]
    InvalidHumanTurn,
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

    pub fn child_activity(
        id: String,
        created_at: String,
        from: String,
        activity: ChildControlActivity,
    ) -> Self {
        Self {
            id,
            role: ChatRole::User,
            text: String::new(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at,
            from: Some(from),
            body: None,
            activity: Some(activity),
        }
    }

    pub fn validate(&self) -> Result<(), ChatTurnError> {
        if self.activity.is_some() {
            if !self.text.is_empty() || !self.items.is_empty() || self.body.is_some() {
                return Err(ChatTurnError::MixedActivity);
            }
            if self.role != ChatRole::User
                || self.status != Lifecycle::Completed
                || self.from.is_none()
            {
                return Err(ChatTurnError::InvalidActivityEnvelope);
            }
        } else if self.role == ChatRole::User
            && (self.status != Lifecycle::Completed
                || !self.items.is_empty()
                || self.body.is_some())
        {
            return Err(ChatTurnError::InvalidHumanTurn);
        }
        Ok(())
    }

    /// The one turn-growth rule every projection shares. The listener's
    /// open-turn snapshot, the journal fold (`fold_thread`), and the resident's
    /// adapter (`EventAdapter`) all grow turns through this — a second copy is
    /// the live-vs-replay split-brain the journal exists to kill.
    ///
    /// Prose joins `text`: a `stream` fragment concatenates verbatim, any other
    /// message (a `final_answer`, an untagged span) joins newline-separated.
    /// The one exception is `commentary` — operational narration the provider
    /// tagged as process, not conclusion. It stays a discrete item so the
    /// surface can curate it behind a disclosure (see Swift `turnPresentation`);
    /// folding it into `text` erased the tag and left that curation with nothing
    /// to fold. Every non-message item appends to `items` as before.
    pub fn absorb_item(&mut self, item: ConversationItem) {
        if let ConversationItem::Message { text, phase, .. } = &item {
            match phase.as_deref() {
                Some("stream") => {
                    self.text.push_str(text);
                    return;
                }
                Some("commentary") => {}
                _ => {
                    self.push_text(text);
                    return;
                }
            }
        }
        self.items.push(item);
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

impl<'de> Deserialize<'de> for ChatTurn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            id: String,
            role: ChatRole,
            text: String,
            status: Lifecycle,
            items: Vec<ConversationItem>,
            created_at: String,
            from: Option<String>,
            body: Option<BodyProvenance>,
            activity: Option<ChildControlActivity>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let turn = Self {
            id: wire.id,
            role: wire.role,
            text: wire.text,
            status: wire.status,
            items: wire.items,
            created_at: wire.created_at,
            from: wire.from,
            body: wire.body,
            activity: wire.activity,
        };
        turn.validate().map_err(serde::de::Error::custom)?;
        Ok(turn)
    }
}

struct ActivityFields {
    kind: ChildActivityKind,
    title: String,
    summary: String,
}

fn task_activity_fields(event: &TaskEventKind) -> ActivityFields {
    match event {
        TaskEventKind::Started => activity(ChildActivityKind::StateChanged, "Task started", ""),
        TaskEventKind::BodyHandedOff { handoff } => activity(
            ChildActivityKind::StateChanged,
            &format!(
                "Task body handed off: {} → {}",
                handoff.from_agent, handoff.to_agent
            ),
            &handoff.reason,
        ),
        TaskEventKind::BodyLeaseChanged { process } => body_lease_activity("Task", process),
        TaskEventKind::BodyRecoveryAttempted {
            attempt, reason, ..
        } => activity(
            ChildActivityKind::StateChanged,
            &format!("Task body recovered automatically (attempt {attempt})"),
            reason,
        ),
        TaskEventKind::StatusChanged { to, reason, .. } => activity(
            ChildActivityKind::StateChanged,
            &format!("Task is {}", to.as_str()),
            reason,
        ),
        TaskEventKind::Progress { summary } => {
            activity(ChildActivityKind::StateChanged, "Task progress", summary)
        }
        TaskEventKind::PrStarted {
            sequence, branch, ..
        } => activity(
            ChildActivityKind::StateChanged,
            &format!("Started PR {sequence}"),
            branch,
        ),
        TaskEventKind::PrOpened { number, url, .. } => activity(
            ChildActivityKind::PrOpened,
            &format!("Opened PR #{number}"),
            url,
        ),
        TaskEventKind::PrMerged { number, url, .. } => activity(
            ChildActivityKind::StateChanged,
            &format!("Merged PR #{number}"),
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
        ProjectEventKind::BodyHandedOff { handoff } => activity(
            ChildActivityKind::StateChanged,
            &format!(
                "Project body handed off: {} → {}",
                handoff.from_agent, handoff.to_agent
            ),
            &handoff.reason,
        ),
        ProjectEventKind::BodyLeaseChanged { process } => body_lease_activity("Project", process),
        ProjectEventKind::StatusChanged { to, reason, .. } => activity(
            ChildActivityKind::StateChanged,
            &format!("Project is {}", to.as_str()),
            reason,
        ),
        ProjectEventKind::TaskObserved { event, .. } => task_activity_fields(event),
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

fn body_lease_activity(subject: &str, process: &ChildProcessGeneration) -> ActivityFields {
    let summary = match process.outcome.as_ref() {
        Some(ChildBodyOutcome::Completed) => "completed",
        Some(ChildBodyOutcome::Interrupted { reason })
        | Some(ChildBodyOutcome::Failed { reason })
        | Some(ChildBodyOutcome::Lost { reason })
        | Some(ChildBodyOutcome::Superseded { reason })
        | Some(ChildBodyOutcome::LegacyStopped { reason }) => reason,
        None => "",
    };
    activity(
        ChildActivityKind::StateChanged,
        &format!(
            "{subject} body generation {} is {}",
            process.generation,
            process.state.as_str()
        ),
        summary,
    )
}

fn activity(kind: ChildActivityKind, title: &str, summary: &str) -> ActivityFields {
    ActivityFields {
        kind,
        title: title.to_string(),
        summary: summary.to_string(),
    }
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
    fn child_activity_envelope_rejects_mixed_conversation_content() {
        let activity = ChildControlActivity {
            id: "task-ts_example-1".to_string(),
            subject: ChildActivitySubject::Task,
            subject_id: "INF-123".to_string(),
            session_id: "ts_example".to_string(),
            kind: ChildActivityKind::StateChanged,
            title: "Task started".to_string(),
            summary: String::new(),
        };
        let turn = ChatTurn::child_activity(
            "turn-activity".to_string(),
            "2026-07-13T20:00:00Z".to_string(),
            "Task INF-123".to_string(),
            activity,
        );
        let mut value = serde_json::to_value(turn).expect("serialize activity turn");
        value["text"] = serde_json::Value::String("mixed prose".to_string());

        let error = serde_json::from_value::<ChatTurn>(value).expect_err("reject mixed activity");
        assert!(error.to_string().contains("cannot also carry prose"));
    }

    #[test]
    fn turn_delta_fixture_round_trips() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/turn_delta.json"
        ));
        let delta: TurnDelta = serde_json::from_str(fixture).expect("decode turn_delta fixture");
        assert_eq!(delta.turn_id, "turn-3");
        match &delta.item {
            ConversationItem::Message { id, text, phase } => {
                assert_eq!(id, "text-7");
                assert_eq!(phase.as_deref(), Some("stream"));
                assert!(text.contains("edge case"));
            }
            other => panic!("expected a stream message item, got {other:?}"),
        }
        // Applying the delta grows a turn exactly as the listener's fold does.
        let mut turn = ChatTurn::user("turn-3".into(), String::new());
        turn.role = ChatRole::Assistant;
        turn.push_text("so ");
        turn.absorb_item(delta.item.clone());
        assert_eq!(turn.text, "so the parser handles the edge case now.");
        assert!(
            turn.items.is_empty(),
            "a stream message joins text, not items"
        );

        // Round-trips: re-serialize and decode to the same value.
        let reencoded = serde_json::to_string(&delta).expect("serialize");
        let again: TurnDelta = serde_json::from_str(&reencoded).expect("decode");
        assert_eq!(again, delta);
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
    fn absorb_item_keeps_commentary_as_a_curatable_item() {
        // The provider tags operational narration `commentary`; it must survive
        // the fold as a discrete item so the surface can curate it. Folding it
        // into `text` (as every message once did) erased the tag.
        let mut turn = ChatTurn::user("turn-10".into(), String::new());
        turn.role = ChatRole::Assistant;
        turn.absorb_item(ConversationItem::Message {
            id: "m-0".into(),
            text: "I'm using `wave_clarify` to audit the plan.".into(),
            phase: Some("commentary".into()),
        });
        turn.absorb_item(ConversationItem::Message {
            id: "m-1".into(),
            text: "Clarification complete.".into(),
            phase: Some("final_answer".into()),
        });

        // The conclusion is the prose; the narration waits in items, untangled
        // from the text so `turnPresentation` can fold it behind a disclosure.
        assert_eq!(turn.text, "Clarification complete.");
        assert_eq!(turn.items.len(), 1);
        match &turn.items[0] {
            ConversationItem::Message { text, phase, .. } => {
                assert_eq!(phase.as_deref(), Some("commentary"));
                assert!(text.contains("wave_clarify"));
            }
            other => panic!("expected the commentary message in items, got {other:?}"),
        }
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
}
