//! Cached GitHub identity values, independent of identity acquisition.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GithubPerson {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

/// A cached answer: `person` is `None` when GitHub knew no user for the email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedGithubPerson {
    pub person: Option<GithubPerson>,
    pub checked_at: i64,
}
