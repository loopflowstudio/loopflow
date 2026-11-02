//! Shared Task discussion, independent of its synchronization transport.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskComment {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    pub body: String,
    pub author: TaskCommentAuthor,
    pub created_at: Option<String>,
}

/// Who wrote a comment. Person attribution survives local saves and provider acquisition;
/// integrations are distinct from participant direction. An agent's direction
/// is the Session's, written for its requester.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskCommentAuthor {
    Person {
        name: Option<String>,
    },
    Integration,
    Session {
        session_id: String,
        requester: Option<String>,
    },
}

impl std::fmt::Display for TaskCommentAuthor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Person { name: Some(name) } => f.write_str(name),
            Self::Person { name: None } => f.write_str("attribution unresolved"),
            Self::Integration => f.write_str("integration"),
            Self::Session {
                session_id,
                requester: Some(requester),
            } => write!(f, "Session {session_id} for {requester}"),
            Self::Session {
                session_id,
                requester: None,
            } => write!(f, "Session {session_id}"),
        }
    }
}

impl TaskCommentAuthor {
    /// The author as the store and peer edits keep it. Released readers know
    /// no Session author, so a Session's comment keeps its requester as the
    /// person, and its body's Session marker names the Session.
    pub fn stored(&self) -> Self {
        match self {
            Self::Session { requester, .. } => Self::Person {
                name: requester.clone(),
            },
            author => author.clone(),
        }
    }

    /// A stored author read with its comment: a person's comment whose body
    /// names a Session is that Session's, written for the person.
    pub fn read(self, body: &str) -> Self {
        match (self, comment_session(body)) {
            (Self::Person { name }, Some(session_id)) => Self::Session {
                session_id,
                requester: name,
            },
            (author, _) => author,
        }
    }
}

/// The marker naming the Session that wrote a steer, so providers round-trip it.
pub const SESSION_MARKER: &str = "<!-- loopflow-session:";

/// The Session a steer's body names as its author.
fn comment_session(body: &str) -> Option<String> {
    body.split_once(SESSION_MARKER)
        .and_then(|(_, rest)| rest.split_once(" -->"))
        .map(|(id, _)| id.trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// The saved Task thread, its pending deliveries and any failed refresh.
/// Partial provider reads never replace the retained thread.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskComments {
    pub identifier: String,
    pub comments: Vec<TaskComment>,
    pub pending_sync: Vec<String>,
    /// Losing local comment bodies; the thread contains the adopted Linear values.
    pub conflicts: std::collections::BTreeMap<String, String>,
    pub refresh_error: Option<String>,
}
