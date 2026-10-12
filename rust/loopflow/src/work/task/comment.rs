//! Shared Task discussion, independent of its synchronization transport.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskComment {
    pub id: String,
    pub body: String,
    pub author: TaskCommentAuthor,
    pub created_at: Option<String>,
}

/// Who wrote a comment. Person attribution survives local saves and provider acquisition;
/// integrations are distinct from participant direction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskCommentAuthor {
    Person { name: Option<String> },
    Integration,
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
