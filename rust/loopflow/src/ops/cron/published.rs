//! Crons are private by default; `lf cron publish` makes one visible to
//! teammates. Only published crons reach Git: each Machine writes the ones it
//! publishes for a repository to its own ref, `refs/loopflow/crons/<machine-id>`
//! on `origin`, independent of the planning transport. A ref has exactly one
//! writer, so publication is a plain force push, and a Machine that publishes
//! nothing has no ref. Published crons are display only: nothing installs or
//! runs from them. The launchd jobs installed on a Machine are its truth.
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{CronTargetKind, InstalledCron};
use crate::durable::MachineId;
use crate::ops::{OpsError, OpsResult};
use crate::planning_git::{checked_git, PlanningGitError};

const PREFIX: &str = "refs/loopflow/crons/";
const FILE: &str = "crons.json";
const REMOTE: &str = "origin";

/// One published cron: what runs, whose it is, and the Machine that runs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedCron {
    pub name: String,
    pub wave: Option<String>,
    pub command: Vec<String>,
    pub schedule: String,
    pub person: Option<String>,
    /// The owner's person key (lowercase Git user.email).
    pub person_key: Option<String>,
    pub machine_id: MachineId,
    pub machine_label: Option<String>,
}

/// A published cron as last fetched, with when its Machine published it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedCron {
    #[serde(flatten)]
    pub cron: PublishedCron,
    pub published_at: i64,
}

/// Who publishes: this Machine and its person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publisher {
    pub person: Option<String>,
    pub person_key: Option<String>,
    pub machine_id: MachineId,
    pub machine_label: Option<String>,
}

fn git_error(error: PlanningGitError) -> OpsError {
    OpsError::Message(match error {
        PlanningGitError::Command { operation, code } => {
            format!("cron publication: git {operation} failed (exit {code:?})")
        }
        PlanningGitError::Timeout(operation) => {
            format!("cron publication: git {operation} timed out")
        }
        other => format!("cron publication: {other}"),
    })
}

fn git(repo: &Path, operation: &'static str, args: &[&str], input: &[u8]) -> OpsResult<String> {
    checked_git(repo, operation, args, input)
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .map_err(git_error)
}

fn has_remote(repo: &Path) -> bool {
    git(repo, "find remote", &["remote"], &[])
        .is_ok_and(|remotes| remotes.lines().any(|remote| remote == REMOTE))
}

/// The crons this Machine publishes for `repo`.
pub fn published_from(
    repo: &Path,
    installed: &[InstalledCron],
    publisher: &Publisher,
) -> OpsResult<Vec<PublishedCron>> {
    let repo = repo.canonicalize()?;
    Ok(installed
        .iter()
        .filter(|cron| {
            cron.published
                && cron.target_kind != CronTargetKind::Repository
                && cron.repo.canonicalize().ok().as_deref() == Some(repo.as_path())
        })
        .map(|cron| PublishedCron {
            name: cron.flow.clone(),
            wave: (!cron.wave.is_empty()).then(|| cron.wave.clone()),
            command: cron.command.clone(),
            schedule: cron.schedule.clone(),
            person: publisher.person.clone(),
            person_key: publisher.person_key.clone(),
            machine_id: publisher.machine_id.clone(),
            machine_label: publisher.machine_label.clone(),
        })
        .collect())
}

/// Replace this Machine's published crons on `origin`. With none left, its
/// ref is deleted. Returns false when there is no `origin` to publish to.
pub fn publish(repo: &Path, machine: &MachineId, crons: &[PublishedCron]) -> OpsResult<bool> {
    let reference = format!("{PREFIX}{machine}");
    let commit = if crons.is_empty() {
        let _ = git(
            repo,
            "drop local ref",
            &["update-ref", "-d", &reference],
            &[],
        );
        None
    } else {
        let commit = write_commit(repo, crons)?;
        git(
            repo,
            "save crons",
            &["update-ref", &reference, &commit],
            &[],
        )?;
        Some(commit)
    };
    if !has_remote(repo) {
        return Ok(false);
    }
    let refspec = match commit {
        Some(commit) => format!("+{commit}:{reference}"),
        None => {
            // Deleting a ref the remote lacks fails, so look first.
            let remote = git(
                repo,
                "discover",
                &["ls-remote", "--refs", REMOTE, &reference],
                &[],
            )?;
            if remote.is_empty() {
                return Ok(true);
            }
            format!(":{reference}")
        }
    };
    git(
        repo,
        "push",
        &["push", "--no-verify", REMOTE, &refspec],
        &[],
    )?;
    Ok(true)
}

fn write_commit(repo: &Path, crons: &[PublishedCron]) -> OpsResult<String> {
    let bytes =
        serde_json::to_vec_pretty(crons).map_err(|error| OpsError::Message(error.to_string()))?;
    let blob = git(
        repo,
        "write crons",
        &["hash-object", "-w", "--stdin"],
        &bytes,
    )?;
    let tree = git(
        repo,
        "write tree",
        &["mktree"],
        format!("100644 blob {blob}\t{FILE}\n").as_bytes(),
    )?;
    git(
        repo,
        "write commit",
        &["commit-tree", &tree, "-m", "Published crons"],
        &[],
    )
}

/// Fetch every Machine's published crons into the local `refs/loopflow/crons/`,
/// pruning Machines that stopped publishing. Without `origin`, nothing changes.
pub fn fetch(repo: &Path) -> OpsResult<()> {
    if !has_remote(repo) {
        return Ok(());
    }
    git(
        repo,
        "fetch",
        &[
            "fetch",
            "--prune",
            "--no-tags",
            "--no-write-fetch-head",
            "--no-recurse-submodules",
            REMOTE,
            &format!("+{PREFIX}*:{PREFIX}*"),
        ],
        &[],
    )
    .map(|_| ())
}

/// Every published cron as last observed, other than `except`'s (this Machine).
/// A ref that does not parse is skipped: another lf version may have written it.
pub fn observed(repo: &Path, except: &MachineId) -> OpsResult<Vec<ObservedCron>> {
    let refs = git(
        repo,
        "list crons",
        &[
            "for-each-ref",
            "--format=%(refname) %(committerdate:unix)",
            PREFIX,
        ],
        &[],
    )?;
    let mut observed = Vec::new();
    for line in refs.lines() {
        let Some((reference, at)) = line.split_once(' ') else {
            continue;
        };
        if reference.strip_prefix(PREFIX) == Some(except.as_str()) {
            continue;
        }
        let Ok(content) = git(
            repo,
            "read crons",
            &["show", &format!("{reference}:{FILE}")],
            &[],
        ) else {
            continue;
        };
        let Ok(crons) = serde_json::from_str::<Vec<PublishedCron>>(&content) else {
            continue;
        };
        let published_at = at.parse().unwrap_or_default();
        observed.extend(
            crons
                .into_iter()
                .map(|cron| ObservedCron { cron, published_at }),
        );
    }
    let order = |observed: &ObservedCron| {
        let cron = &observed.cron;
        (
            cron.person.clone(),
            cron.machine_id.to_string(),
            cron.name.clone(),
        )
    };
    observed.sort_by_key(order);
    Ok(observed)
}

#[cfg(test)]
mod tests {
    use super::{fetch, observed, publish, PublishedCron};
    use crate::durable::MachineId;
    use std::path::Path;
    use std::process::Command;

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(repo)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn cron(person: &str, machine: &MachineId) -> PublishedCron {
        PublishedCron {
            name: "nightly".into(),
            wave: Some("infra".into()),
            command: vec![
                "--wave".into(),
                "infra".into(),
                "flow".into(),
                "telemetry-daily".into(),
            ],
            schedule: "0 0 9 * * *".into(),
            person: Some(person.into()),
            person_key: Some(format!("{}@example.com", person.to_lowercase())),
            machine_id: machine.clone(),
            machine_label: Some(format!("{}-mbp", person.to_lowercase())),
        }
    }

    #[test]
    fn teammates_see_published_crons_until_they_are_unpublished() {
        let temp = tempfile::tempdir().unwrap();
        let origin = temp.path().join("origin.git");
        git(temp.path(), &["init", "--bare", origin.to_str().unwrap()]);
        let mut clones = Vec::new();
        for name in ["maya", "lee"] {
            let clone = temp.path().join(name);
            git(
                temp.path(),
                &["clone", origin.to_str().unwrap(), clone.to_str().unwrap()],
            );
            clones.push(clone);
        }
        let (maya, lee) = (MachineId::new(), MachineId::new());
        assert!(publish(&clones[0], &maya, &[cron("Maya", &maya)]).unwrap());
        fetch(&clones[1]).unwrap();
        let seen = observed(&clones[1], &lee).unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].cron, cron("Maya", &maya));
        assert!(seen[0].published_at > 0);
        // A Machine never lists its own crons as a teammate's.
        assert!(observed(&clones[0], &maya).unwrap().is_empty());

        // Unpublishing the last cron deletes the ref; teammates stop seeing it.
        publish(&clones[0], &maya, &[]).unwrap();
        fetch(&clones[1]).unwrap();
        assert!(observed(&clones[1], &lee).unwrap().is_empty());
    }
}
