//! The `session` namespace: direction for conversations. `session.steer`
//! saves a steer that one Session, every Session of a Wave, every Session of
//! a Task or every Session of a repository reads at its next provider
//! boundary, without starting a turn. The Task scope is what `task.steer`
//! delivers through ([`super::task::steer`]); its entry joins the Task's thread.

use std::path::Path;

use super::{Error, ErrorCode, Result};
use crate::steer::Steer;
use crate::store::SharedStore;

/// Whose Sessions a steer reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteerScope<'a> {
    Session(&'a str),
    /// Every Session on the Wave, by name.
    Wave(&'a str),
    /// Every Session on the Task, now or later: the steer is the Task's, and
    /// its text joins the Task's thread as a comment.
    Task {
        issue: &'a str,
        wave: Option<&'a str>,
    },
    /// Every Session of the repository containing the directory.
    Repository,
}

/// `session.steer`: save `message` for `scope`, as `lf session steer` does,
/// and return the saved steer. Delivery is each Session's, at its next
/// boundary; `lf session steers <id>` reads it.
pub async fn steer(
    store: &SharedStore,
    directory: &Path,
    scope: SteerScope<'_>,
    message: &str,
) -> Result<Steer> {
    let author = crate::config::participant_name_in(directory)
        .map_err(written)?
        .unwrap_or_else(|| "attribution unresolved".into());
    match scope {
        SteerScope::Session(id) => {
            let session = crate::ops::human_session::session_by_id(store, id)
                .await
                .map_err(written)?
                .ok_or_else(|| {
                    Error::new(ErrorCode::NotFound, format!("Session {id} not found"))
                        .with(serde_json::json!({ "session": id }))
                })?;
            store
                .sqlite
                .save_session_steer(&session.id, &author, message)
                .map_err(written)
        }
        SteerScope::Wave(wave) => {
            crate::lf::commands::waves::save_steer(store, wave, &author, message)
                .await
                .map_err(written)
        }
        SteerScope::Task { issue, wave } => {
            if message.trim().is_empty() {
                return Err(Error::new(
                    ErrorCode::Invalid,
                    "Task comment cannot be empty",
                ));
            }
            // The comment's transaction derives the Task's steer from the entry.
            let thread =
                crate::ops::task::task_comment_async(directory, wave, issue, Some(message), true)
                    .await
                    .map_err(written)?;
            let comment = thread.comments.last().ok_or_else(|| {
                Error::new(
                    ErrorCode::OutcomeUnknown,
                    format!("the thread of {issue} does not show the saved entry"),
                )
            })?;
            store
                .sqlite
                .steer_by_source(&format!("comment:{}", comment.id), "local")
                .map_err(written)?
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::OutcomeUnknown,
                        format!("comment {} of {issue} derived no steer", comment.id),
                    )
                })
        }
        SteerScope::Repository => {
            let root = crate::repo::discover_repo_root(directory)
                .map_err(written)?
                .unwrap_or_else(|| directory.to_path_buf());
            let repo = crate::repository::CanonicalRepo::discover(&root).map_err(written)?;
            store
                .sqlite
                .save_repository_steer(&repo, &author, message)
                .map_err(written)
        }
    }
}

fn written(error: impl Into<anyhow::Error>) -> Error {
    Error::written(&error.into())
}
