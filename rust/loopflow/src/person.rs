//! A Person: the one identity Loopflow publishes. Each person's Machines write
//! `refs/loopflow/people/<key>` on `origin`; every Machine fetches all of them
//! into its local `people` cache. Machines are never published.

use std::path::Path;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

const PEOPLE: &str = "refs/loopflow/people/";

#[derive(Debug, thiserror::Error)]
pub enum PersonError {
    #[error("{0} is not an email Git can name a ref after")]
    InvalidKey(String),
    // Git diagnostics may contain credential-bearing URLs.
    #[error("people Git {operation} failed (exit {code:?})")]
    Git {
        operation: &'static str,
        code: Option<i32>,
    },
    #[error("people Git I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("person record {reference} is unreadable: {error}")]
    Unreadable {
        reference: String,
        error: serde_json::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonProvider {
    Linear,
    Asana,
}

/// A person's identity on a planning provider, kept where the email alone
/// cannot find it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub provider: PersonProvider,
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonRecord {
    /// Lowercase email.
    pub key: String,
    /// Display only.
    pub name: String,
    pub accounts: Vec<ProviderAccount>,
    /// Unix seconds of the last change.
    pub updated_at: i64,
}

/// The person key for `email`: trimmed and lowercased. `None` when Git could
/// not name a ref after it.
pub fn person_key(email: &str) -> Option<String> {
    let key = email.trim().to_lowercase();
    let valid = key.contains('@')
        && !key.starts_with('.')
        && !key.ends_with('.')
        && !key.ends_with(".lock")
        && !key.contains("..")
        && !key.contains("@{")
        && !key
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "~^:?*[\\/".contains(c));
    valid.then_some(key)
}

/// `remote` updated with what this Machine knows about its person: its name,
/// and its account for each provider it is signed in to. Accounts on other
/// providers are kept. `None` when nothing would change.
pub fn merge(remote: Option<&PersonRecord>, mine: &PersonRecord) -> Option<PersonRecord> {
    let mut accounts: Vec<ProviderAccount> = remote
        .map(|record| record.accounts.clone())
        .unwrap_or_default();
    for account in &mine.accounts {
        accounts.retain(|kept| kept.provider != account.provider);
        accounts.push(account.clone());
    }
    accounts.sort_by_key(|account| account.provider);
    if let Some(remote) = remote {
        if remote.name == mine.name && remote.accounts == accounts {
            return None;
        }
    }
    Some(PersonRecord {
        key: mine.key.clone(),
        name: mine.name.clone(),
        accounts,
        updated_at: mine.updated_at,
    })
}

/// Publish what this Machine knows about its person to `remote`, merged into
/// the record there. The push is leased against the record it merged; a lost
/// lease refetches and retries once. Returns the record when it changed.
pub fn publish(
    repo: &Path,
    remote: &str,
    mine: &PersonRecord,
) -> Result<Option<PersonRecord>, PersonError> {
    let key = person_key(&mine.key).ok_or_else(|| PersonError::InvalidKey(mine.key.clone()))?;
    let reference = format!("{PEOPLE}{key}");
    let mut attempt = 0;
    loop {
        attempt += 1;
        let listed = git(
            repo,
            "list",
            &["ls-remote", "--refs", remote, &reference],
            &[],
        )?;
        let old = String::from_utf8_lossy(&listed)
            .split_whitespace()
            .next()
            .map(str::to_string);
        let remote_record = match &old {
            Some(_) => {
                let refspec = format!("+{reference}:{reference}");
                git(repo, "fetch", &fetch_args(remote, &refspec), &[])?;
                Some(read_record(repo, &reference)?)
            }
            None => None,
        };
        let Some(merged) = merge(remote_record.as_ref(), mine) else {
            return Ok(None);
        };
        let json = serde_json::to_vec_pretty(&merged).expect("a person record serializes");
        let blob = object_id(&git(
            repo,
            "write",
            &["hash-object", "-w", "--stdin"],
            &json,
        )?);
        let tree = object_id(&git(
            repo,
            "write",
            &["mktree"],
            format!("100644 blob {blob}\tperson.json\n").as_bytes(),
        )?);
        let mut args = vec!["commit-tree", tree.as_str()];
        if let Some(old) = &old {
            args.extend(["-p", old.as_str()]);
        }
        let commit = object_id(&git(repo, "write", &args, b"Publish person\n")?);
        // An empty lease requires the ref to be absent.
        let lease = format!(
            "--force-with-lease={reference}:{}",
            old.as_deref().unwrap_or("")
        );
        let target = format!("{commit}:{reference}");
        match git(
            repo,
            "push",
            &["push", "--porcelain", &lease, remote, &target],
            &[],
        ) {
            Ok(_) => {
                git(repo, "save", &["update-ref", &reference, &commit], &[])?;
                return Ok(Some(merged));
            }
            Err(_) if attempt == 1 => continue,
            Err(error) => return Err(error),
        }
    }
}

/// Every person record on `remote`. A record that does not parse is skipped
/// with a warning rather than hiding everyone else.
pub fn fetch(repo: &Path, remote: &str) -> Result<Vec<PersonRecord>, PersonError> {
    let refspec = format!("+{PEOPLE}*:{PEOPLE}*");
    let mut args = fetch_args(remote, &refspec);
    args.insert(1, "--prune");
    git(repo, "fetch", &args, &[])?;
    let listed = git(
        repo,
        "list",
        &["for-each-ref", "--format=%(refname)", PEOPLE],
        &[],
    )?;
    let mut records = Vec::new();
    for reference in String::from_utf8_lossy(&listed).lines() {
        match read_record(repo, reference) {
            Ok(record) => records.push(record),
            Err(error) => tracing::warn!(%error, "person record skipped"),
        }
    }
    Ok(records)
}

fn fetch_args<'a>(remote: &'a str, refspec: &'a str) -> Vec<&'a str> {
    vec![
        "fetch",
        "--no-tags",
        "--no-write-fetch-head",
        "--no-recurse-submodules",
        remote,
        refspec,
    ]
}

fn read_record(repo: &Path, reference: &str) -> Result<PersonRecord, PersonError> {
    let bytes = git(
        repo,
        "read",
        &["cat-file", "blob", &format!("{reference}:person.json")],
        &[],
    )?;
    serde_json::from_slice(&bytes).map_err(|error| PersonError::Unreadable {
        reference: reference.to_string(),
        error,
    })
}

fn object_id(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn git(
    repo: &Path,
    operation: &'static str,
    args: &[&str],
    input: &[u8],
) -> Result<Vec<u8>, PersonError> {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Loopflow")
        .env("GIT_AUTHOR_EMAIL", "people@loopflow.invalid")
        .env("GIT_COMMITTER_NAME", "Loopflow")
        .env("GIT_COMMITTER_EMAIL", "people@loopflow.invalid")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("stdin is piped");
        stdin.write_all(input)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(PersonError::Git {
            operation,
            code: output.status.code(),
        });
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::{fetch, merge, person_key, publish, PersonProvider, PersonRecord, ProviderAccount};

    fn record(accounts: &[(PersonProvider, &str)]) -> PersonRecord {
        PersonRecord {
            key: "dustin@example.com".into(),
            name: "Dustin".into(),
            accounts: accounts
                .iter()
                .map(|(provider, id)| ProviderAccount {
                    provider: *provider,
                    id: id.to_string(),
                    email: None,
                })
                .collect(),
            updated_at: 1,
        }
    }

    #[test]
    fn person_keys_are_lowercase_emails_git_can_name() {
        assert_eq!(
            person_key(" Dustin@Example.com ").as_deref(),
            Some("dustin@example.com")
        );
        for invalid in ["dustin", ".d@example.com", "d@example.lock", "d..x@e.com"] {
            assert_eq!(person_key(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn merging_keeps_other_providers_and_reports_no_change() {
        let linear = record(&[(PersonProvider::Linear, "lin-1")]);
        let asana = record(&[(PersonProvider::Asana, "as-1")]);
        let both = merge(Some(&linear), &asana).unwrap();
        assert_eq!(both.accounts.len(), 2);
        assert_eq!(merge(Some(&both), &asana), None);
        assert_eq!(merge(Some(&both), &record(&[])), None);
        let moved = merge(Some(&both), &record(&[(PersonProvider::Linear, "lin-2")])).unwrap();
        assert_eq!(moved.accounts[0].id, "lin-2");
        assert_eq!(moved.accounts[1].id, "as-1");
    }

    fn git(dir: &std::path::Path, args: &[&str]) {
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
    fn two_machines_of_one_person_converge_on_one_record() {
        let root = tempfile::tempdir().unwrap();
        let origin = root.path().join("origin.git");
        std::fs::create_dir(&origin).unwrap();
        git(&origin, &["init", "-q", "--bare"]);
        let machine = |name: &str| {
            let dir = root.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            git(&dir, &["init", "-q"]);
            git(&dir, &["remote", "add", "origin", origin.to_str().unwrap()]);
            dir
        };
        let (laptop, mini) = (machine("laptop"), machine("mini"));

        let linear = record(&[(PersonProvider::Linear, "lin-1")]);
        assert!(publish(&laptop, "origin", &linear).unwrap().is_some());
        let asana = record(&[(PersonProvider::Asana, "as-1")]);
        let both = publish(&mini, "origin", &asana).unwrap().unwrap();
        assert_eq!(both.accounts.len(), 2);
        // Nothing new to say publishes nothing.
        assert_eq!(publish(&laptop, "origin", &linear).unwrap(), None);

        assert_eq!(fetch(&laptop, "origin").unwrap(), [both]);
    }
}
