//! The `session` namespace: direction for conversations. `session.steer`
//! saves a steer that one Session, every Session of a Wave, or every Session of
//! a repository reads at its next provider
//! boundary, without starting a turn.

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
    /// Every Session of the repository containing the directory.
    Repository,
}

/// `session.steer`: save `message` for `scope`, as `lf steer` does,
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
        SteerScope::Wave(wave) => crate::steer::save_steer(store, wave, &author, message)
            .await
            .map_err(written),
        SteerScope::Repository => {
            let root = crate::repo::discover_repo_root(directory)
                .map_err(written)?
                .unwrap_or_else(|| directory.to_path_buf());
            let repo = crate::repository::discover(&root).map_err(written)?;
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
/// Who reads a steer saved for `session`, with no turn of its own to run it.
pub fn steer_reader(
    sqlite: &crate::store::sqlite::SqliteStore,
    session: &crate::session::LfSession,
) -> anyhow::Result<String> {
    let id = &session.id;
    Ok(
        if crate::ops::human_session::agent_running(sqlite, session)? {
            live_reader(id.as_str())
        } else if session.interactive {
            format!("the Machine server starts {id} to read it")
        } else if session.graph_lf_process_id.is_some() {
            format!("{id} is a finished worker; nothing reads it until its Graph continues it")
        } else {
            format!("{id} is a finished batch; nothing reads it")
        },
    )
}

fn live_reader(reader: &str) -> String {
    format!("{reader} reads it at its next boundary")
}

/// The first reader a Task steer can reach on this Machine.
pub(super) fn task_steer_reader(
    sqlite: &crate::store::sqlite::SqliteStore,
    task: &crate::work::task::Task,
) -> anyhow::Result<String> {
    use loopflow_storage::sqlite::task_work::OpenWork;
    if sqlite.task_state(&task.id)?.is_terminal() {
        return Ok(format!("{} is finished", task.identifier));
    }
    for work in sqlite.task_open_work(&task.id)? {
        if let OpenWork::Session { id, .. } = work {
            if let Some(session) = sqlite.session(id.as_str())? {
                if crate::ops::human_session::agent_running(sqlite, &session)? {
                    let reader = if session.graph_lf_process_id.is_some() && !session.interactive {
                        format!("{}'s worker", task.identifier)
                    } else {
                        session.id.to_string()
                    };
                    return Ok(live_reader(&reader));
                }
            }
        }
    }
    let scope = loopflow_storage::session::PrimaryScope::Task(task.id.clone());
    if let Some(primary) = sqlite
        .primary_session(&scope)?
        .filter(|session| session.completed_at.is_none())
    {
        return steer_reader(sqlite, &primary);
    }
    Ok(format!("nothing reads it until {} runs", task.identifier))
}
