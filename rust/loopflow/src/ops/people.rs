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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use crate::ops::planning_sync::PlanningSync;
    use crate::store::StorageConfig;

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    #[test]
    fn git_planning_sync_lists_you_without_refresh() {
        let root = tempfile::tempdir().unwrap();
        let origin = root.path().join("origin.git");
        let repo = root.path().join("repo");
        std::fs::create_dir(&origin).unwrap();
        std::fs::create_dir(&repo).unwrap();
        git(&origin, &["init", "-q", "--bare"]);
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "Dustin@Example.com"]);
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let config = StorageConfig::sqlite(root.path().join("loopflow.db"));
        let store = Arc::new(
            runtime
                .block_on(crate::store::open_ephemeral_store(&config))
                .unwrap(),
        );
        let sync =
            PlanningSync::start_watch(store.clone(), repo.to_string_lossy().into_owned()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while store.sqlite.people().unwrap().is_empty() {
            assert!(Instant::now() < deadline, "people never synchronized");
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(sync);

        let people = super::list(&store.sqlite, &repo).unwrap();
        assert_eq!(people.len(), 1);
        assert_eq!(people[0].key, "dustin@example.com");
        assert!(people[0].is_you);
    }
}
