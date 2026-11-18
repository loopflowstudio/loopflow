//! Shared Task discussion, independent of its synchronization transport.

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TaskComment {
    #[serde(deserialize_with = "crate::id::deserialize_uuid")]
    pub id: String,
    pub body: String,
    pub author: TaskCommentAuthor,
    pub created_at: Option<String>,
}

/// Who wrote a comment. Person attribution survives local saves and tracker fetches;
/// integrations are distinct from participant direction. An agent's direction
/// is the Session's, written for its requester.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
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
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct TaskComments {
    pub identifier: String,
    pub comments: Vec<TaskComment>,
    pub pending_sync: Vec<String>,
    /// Losing local comment bodies; the thread contains the adopted Linear values.
    pub conflicts: std::collections::BTreeMap<String, String>,
    pub refresh_error: Option<String>,
}

/// Explicit steering carries its marker, whichever account published it.
pub fn is_steer(body: &str) -> bool {
    body.contains("<!-- loopflow-steer:")
}

pub fn is_direction_comment(body: &str, author: Option<&str>) -> bool {
    if body.contains("<!-- loopflow-progress:") {
        return false;
    }
    if is_steer(body) {
        return true;
    }
    author.is_some()
        && !body.contains("<!-- loopflow-")
        && !body.starts_with("PR: ")
        && !body.starts_with("Shipped: ")
        && !body.starts_with("Reteamed by loopflow:")
        && !body.starts_with("[GitHub PR #")
}

pub fn comment_revision_id(id: &str, revision: Option<&str>) -> String {
    match revision {
        Some(revision) => format!("{id}@{revision}"),
        None => id.to_string(),
    }
}

/// The person a comment speaks for. Explicit steering can be published through
/// an integration account: its recorded requester wins over the Linear author.
pub fn comment_requester(body: &str, author_name: Option<&str>) -> Option<String> {
    let requester = if is_steer(body) {
        body.split_once("<!-- loopflow-requester:")
            .and_then(|(_, rest)| rest.split_once(" -->"))
            .and_then(|(name, _)| serde_json::from_str::<String>(name).ok())
    } else {
        author_name.map(str::to_string)
    };
    requester
        .as_deref()
        .and_then(crate::person::normalize_name)
        .or_else(|| author_name.and_then(crate::person::normalize_name))
}

pub fn render_comment(
    id: &str,
    body: &str,
    author_id: Option<&str>,
    author_name: Option<&str>,
) -> String {
    let attribution = comment_requester(body, author_name)
        .map(|name| {
            format!(
                " by {}",
                serde_json::to_string(&name).expect("name is serializable")
            )
        })
        .unwrap_or_default();
    let source = author_id
        .map(|id| format!(" (provider user {id})"))
        .unwrap_or_default();
    format!("Planning comment {id}{attribution}{source}:\n\n{body}")
}

#[cfg(test)]
mod tests {
    use super::{is_direction_comment, render_comment};

    #[test]
    fn agent_progress_never_reenters_direction_even_with_a_quoted_steer() {
        assert!(!is_direction_comment(
            "done <!-- loopflow-progress:run_fixture --> <!-- loopflow-steer:quoted -->",
            Some("publisher"),
        ));
        assert!(is_direction_comment("keep the API", Some("publisher")));
        assert!(is_direction_comment(
            "new instruction <!-- loopflow-steer:explicit -->",
            None
        ));
    }

    #[test]
    fn request_authors_survive_linear_direction_rendering() {
        for name in ["Jack", "Maya"] {
            let rendered = render_comment("one", "prototype", Some("person"), Some(name));
            assert!(rendered.contains(&format!("by \"{name}\" (provider user person)")));
        }
        assert!(!render_comment("one", "prototype", Some("person"), None).contains(" by "));
        let explicit =
            "prototype\n<!-- loopflow-requester:\"Jack\" -->\n<!-- loopflow-steer:one -->";
        let rendered = render_comment("one", explicit, Some("publisher"), Some("Account Owner"));
        assert!(rendered.contains("by \"Jack\""));
        assert!(!rendered.contains("Account Owner"));
        let legacy = render_comment(
            "old",
            "prototype\n<!-- loopflow-steer:old -->",
            Some("publisher"),
            Some("Account Owner"),
        );
        assert!(legacy.contains("by \"Account Owner\""));
    }

    #[test]
    fn direction_attribution_trims_names_and_falls_back_from_an_empty_requester() {
        let body = "fix it <!-- loopflow-requester:\" \" --> <!-- loopflow-steer:one -->";
        assert!(render_comment("one", body, None, Some("  Maya  ")).contains("by \"Maya\""));
        assert!(!render_comment("one", "fix it", None, Some(" \t ")).contains(" by "));
    }

    #[test]
    fn account_identity_does_not_filter_direction() {
        for author in ["person", "publisher"] {
            assert!(is_direction_comment("please fix this", Some(author)));
            assert!(!is_direction_comment("PR: x", Some(author)));
        }
        assert!(!is_direction_comment("bot", None));
    }
}
