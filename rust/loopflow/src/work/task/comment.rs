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
