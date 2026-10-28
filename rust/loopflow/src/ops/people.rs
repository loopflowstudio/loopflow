//! The people who own a repository's Tasks, with their GitHub identity.
//!
//! An owner's claim carries their Git email. GitHub answers which user wrote the
//! repository's commits under that email; the answer is cached in the store, so
//! each email reaches the network once. GitHub knowing no user is cached too and
//! retried after a day; an unavailable or unauthenticated `gh` caches nothing.
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::store::sqlite::SqliteStore;

/// How long GitHub knowing no user for an email stands before it is asked again.
const MISS_RETRY_SECS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubPerson {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
}

/// A Task owner: their claim's email and name, and who they are on GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoPerson {
    pub email: String,
    pub name: Option<String>,
    pub github: Option<GithubPerson>,
}

/// A cached answer: `person` is `None` when GitHub knew no user for the email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CachedGithubPerson {
    pub person: Option<GithubPerson>,
    pub checked_at: i64,
}

/// Every Task owner in `repo`, and the warnings left by lookups that could not
/// reach GitHub. Those people read with no GitHub identity.
pub fn repo_people(store: &SqliteStore, repo: &Path) -> Result<(Vec<RepoPerson>, Vec<String>)> {
    let owners = store.task_owner_emails(&repo.to_string_lossy())?;
    let nwo = crate::git::worktrees::github_repo_nwo(repo);
    let gh = super::pr::gh_available();
    let now = chrono::Utc::now().timestamp();
    let mut warnings = Vec::new();
    // One failed lookup means GitHub is out of reach; the rest read the cache.
    let mut github = nwo.as_ref().filter(|_| gh);
    let mut people = Vec::with_capacity(owners.len());
    for (email, name) in owners {
        let cached = || -> Result<Option<GithubPerson>> {
            Ok(store
                .github_person(&email)?
                .and_then(|cached| cached.person))
        };
        let person = match github {
            Some((owner, repo_name)) => {
                match github_person(store, repo, owner, repo_name, &email, now) {
                    Ok(person) => person,
                    Err(error) => {
                        warnings.push(format!("{email}: {error:#}"));
                        github = None;
                        cached()?
                    }
                }
            }
            None => cached()?,
        };
        people.push(RepoPerson {
            email,
            name,
            github: person,
        });
    }
    if !people.is_empty() {
        if nwo.is_none() {
            warnings.push("origin is not a GitHub repository".into());
        } else if !gh {
            warnings.push("gh CLI not found".into());
        }
    }
    Ok((people, warnings))
}

/// The GitHub user behind `email`, from the cache or else the repository's commits.
pub(crate) fn github_person(
    store: &SqliteStore,
    repo: &Path,
    owner: &str,
    name: &str,
    email: &str,
    now: i64,
) -> Result<Option<GithubPerson>> {
    if let Some(cached) = store.github_person(email)? {
        if cached.person.is_some() || now - cached.checked_at < MISS_RETRY_SECS {
            return Ok(cached.person);
        }
    }
    let person = lookup(repo, owner, name, email)?;
    store.record_github_person(email, person.as_ref(), now)?;
    Ok(person)
}

#[derive(Debug, Deserialize)]
struct GhCommit {
    author: Option<GhUser>,
}

#[derive(Debug, Deserialize)]
struct GhUser {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GhProfile {
    name: Option<String>,
}

fn lookup(repo: &Path, owner: &str, name: &str, email: &str) -> Result<Option<GithubPerson>> {
    let author = format!("author={email}");
    let commits: Vec<GhCommit> = gh_api(
        repo,
        &[
            "-X",
            "GET",
            &format!("repos/{owner}/{name}/commits"),
            "-f",
            &author,
            "-f",
            "per_page=1",
        ],
    )?;
    let Some(user) = commits.into_iter().next().and_then(|commit| commit.author) else {
        return Ok(None);
    };
    let profile: GhProfile = gh_api(repo, &[&format!("users/{}", user.login)])?;
    Ok(Some(GithubPerson {
        login: user.login,
        name: profile.name.filter(|name| !name.is_empty()),
        avatar_url: user.avatar_url,
    }))
}

fn gh_api<T: serde::de::DeserializeOwned>(repo: &Path, args: &[&str]) -> Result<T> {
    let output = Command::new("gh")
        .current_dir(repo)
        .arg("api")
        .args(args)
        .output()
        .context("failed to invoke gh")?;
    if !output.status.success() {
        return Err(anyhow!(
            "gh api {} failed: {}",
            args.join(" "),
            super::util::stderr_from_output(&output)
        ));
    }
    serde_json::from_slice(&output.stdout).context("failed to parse gh api response")
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::{github_person, GithubPerson};
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn an_email_reaches_github_once() {
        let temp = tempfile::tempdir().unwrap();
        let calls = temp.path().join("calls");
        let gh = temp.path().join("gh");
        std::fs::write(
            &gh,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncase \"$*\" in\n\
                 *repos/acme/app/commits*) echo '[{{\"author\":{{\"login\":\"maya\",\"avatar_url\":\"https://a/u/1\"}}}}]' ;;\n\
                 *users/maya*) echo '{{\"name\":\"Maya Lin\"}}' ;;\n\
                 *) exit 1 ;;\nesac\n",
                calls.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let store = SqliteStore::open_ephemeral(&temp.path().join("lf.db")).unwrap();
        let _env = crate::journal::test_env_lock();
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(temp.path().to_path_buf()).chain(std::env::split_paths(&path)),
            )
            .unwrap(),
        );
        let resolve = || github_person(&store, temp.path(), "acme", "app", "maya@example.com", 10);
        let first = resolve();
        let second = resolve();
        std::env::set_var("PATH", path);

        let maya = Some(GithubPerson {
            login: "maya".into(),
            name: Some("Maya Lin".into()),
            avatar_url: "https://a/u/1".into(),
        });
        assert_eq!(first.unwrap(), maya);
        assert_eq!(second.unwrap(), maya);
        let calls = std::fs::read_to_string(calls).unwrap();
        assert_eq!(
            calls.lines().count(),
            2,
            "commits and profile, once: {calls}"
        );
    }
}
