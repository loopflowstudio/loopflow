use serde::{Deserialize, Serialize};

/// Conversation identity and completion, without loading its input or transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSession {
    pub id: loopflow_model::id::LfSessionId,
    pub title: String,
    pub interactive: bool,
    /// The Graph whose step opened its current input.
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid_option")]
    pub graph_lf_process_id: Option<String>,
    pub completed_at: Option<i64>,
}
