//! Keep this person's record on `origin` current and cache everyone else's.

use std::path::Path;

use serde::Serialize;

use super::{OpsError, OpsResult};
use crate::person::{PersonProvider, PersonRecord, ProviderAccount};
use crate::store::rows::now_unix;
use crate::store::sqlite::SqliteStore;

const REMOTE: &str = "origin";

/// One person as `lf person list` shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct PersonView {
    pub key: String,
    pub name: String,
    pub accounts: Vec<ProviderAccount>,
    pub is_you: bool,
}

/// Publish your record when it would change, then cache every record on
/// `origin`. A repository without `origin` has no one to share with.
pub async fn sync(store: &SqliteStore, repo: &Path) -> OpsResult<()> {
    if !has_origin(repo) {
        return Ok(());
    }
    let mine = your_record(repo).await?;
    let repo = repo.to_path_buf();
    let fetched = tokio::task::spawn_blocking(move || {
        if let Some(mine) = mine {
            crate::person::publish(&repo, REMOTE, &mine)?;
        }
        crate::person::fetch(&repo, REMOTE)
    })
    .await
    .map_err(message)?
    .map_err(message)?;
    store.replace_people(&fetched).map_err(message)
}

pub fn list(store: &SqliteStore, repo: &Path) -> OpsResult<Vec<PersonView>> {
    let you = crate::config::participant_email(repo);
    Ok(store
        .people()
        .map_err(message)?
        .into_iter()
        .map(|person| PersonView {
            is_you: you.as_deref() == Some(person.key.as_str()),
            key: person.key,
            name: person.name,
            accounts: person.accounts,
        })
        .collect())
}

/// What this Machine knows about its person: Git's email and your name, and
/// the Linear account it is signed in to. `None` without an email.
async fn your_record(repo: &Path) -> OpsResult<Option<PersonRecord>> {
    let Some(key) = crate::config::participant_email(repo) else {
        return Ok(None);
    };
    let name = crate::config::participant_name()?.unwrap_or_else(|| key.clone());
    let mut accounts = Vec::new();
    if super::linear_observe::connected(&repo.to_string_lossy()) {
        match super::linear::issue_client().await {
            Ok(client) => match client.viewer().await {
                Ok(viewer) => accounts.push(ProviderAccount {
                    provider: PersonProvider::Linear,
                    id: viewer.id,
                    email: Some(viewer.email.to_lowercase()),
                }),
                Err(error) => {
                    tracing::warn!(%error, "Linear viewer unavailable; publishing without it")
                }
            },
            Err(error) => tracing::warn!(%error, "Linear unavailable; publishing without it"),
        }
    }
    Ok(Some(PersonRecord {
        key,
        name,
        accounts,
        updated_at: now_unix(),
    }))
}

fn has_origin(repo: &Path) -> bool {
    std::process::Command::new("git")
        .current_dir(repo)
        .args(["remote", "get-url", REMOTE])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}
