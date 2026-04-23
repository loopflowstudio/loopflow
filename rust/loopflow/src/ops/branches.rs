use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Serialize;

use crate::engine::config::load_config;
use crate::engine::git::{
    current_branch, delete_remote_branch, get_default_branch, is_squash_merged,
};
use crate::engine::naming::wave_name_from_branch;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct BranchListOptions {
    pub user: Option<String>,
    pub wave: Option<String>,
    pub stale: Option<String>,
    pub created_before: Option<String>,
    pub merged: bool,
    pub include_open_prs: bool,
    pub default_user_if_empty: bool,
}

#[derive(Debug, Clone)]
pub struct BranchPruneOptions {
    pub user: Option<String>,
    pub wave: Option<String>,
    pub stale: Option<String>,
    pub created_before: Option<String>,
    pub merged: bool,
    pub include_open_prs: bool,
    pub dry_run: bool,
    pub yes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BranchCandidate {
    pub branch: String,
    pub author: String,
    pub last_commit_date: String,
    pub age_days: i64,
    pub wave: Option<String>,
    pub merged: bool,
    pub open_pr: bool,
    pub protected: bool,
    pub protect_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct BranchFilters {
    user: Option<String>,
    wave: Option<String>,
    stale_days: Option<i64>,
    created_before: Option<NaiveDate>,
    merged: bool,
    include_open_prs: bool,
}

#[derive(Debug, Clone)]
struct RemoteBranchRef {
    branch: String,
    author: String,
    committer_timestamp: i64,
}

pub fn list_branch_candidates(
    repo: &Path,
    options: &BranchListOptions,
) -> OpsResult<Vec<BranchCandidate>> {
    let filters = BranchFilters::from_list_options(repo, options)?;
    collect_branch_candidates(repo, &filters)
}

pub fn prune_branches(
    repo: &Path,
    options: &BranchPruneOptions,
    progress: &impl Progress,
) -> OpsResult<Vec<BranchCandidate>> {
    if !options.has_filter() {
        return Err(OpsError::Message(
            "refusing to prune remote branches without at least one filter".to_string(),
        ));
    }

    let filters = BranchFilters::from_prune_options(repo, options)?;
    let candidates = collect_branch_candidates(repo, &filters)?;
    let targets = candidates
        .into_iter()
        .filter(|branch| !branch.protected)
        .collect::<Vec<_>>();

    if targets.is_empty() {
        progress.status("No deletable remote branches match.");
        return Ok(Vec::new());
    }

    if options.dry_run {
        return Ok(targets);
    }

    let summary = targets
        .iter()
        .map(|candidate| format!("  {}", candidate.branch))
        .collect::<Vec<_>>()
        .join("\n");
    if !options.yes && !progress.confirm(&format!("Delete these remote branches?\n{summary}")) {
        return Err(OpsError::Message("aborted".to_string()));
    }

    for candidate in &targets {
        progress.status(&format!("Deleting origin/{}...", candidate.branch));
        delete_remote_branch(repo, "origin", &candidate.branch)?;
    }

    Ok(targets)
}

impl BranchPruneOptions {
    fn has_filter(&self) -> bool {
        self.user.is_some()
            || self.wave.is_some()
            || self.stale.is_some()
            || self.created_before.is_some()
            || self.merged
    }
}

impl BranchFilters {
    fn from_list_options(repo: &Path, options: &BranchListOptions) -> OpsResult<Self> {
        let no_filters = options.user.is_none()
            && options.wave.is_none()
            && options.stale.is_none()
            && options.created_before.is_none()
            && !options.merged;
        let user = if no_filters && options.default_user_if_empty {
            Some(resolve_user(repo, "@me")?)
        } else {
            options
                .user
                .as_deref()
                .map(|user| resolve_user(repo, user))
                .transpose()?
        };
        Self::new(
            user,
            options.wave.clone(),
            options.stale.as_deref(),
            options.created_before.as_deref(),
            options.merged,
            options.include_open_prs,
        )
    }

    fn from_prune_options(repo: &Path, options: &BranchPruneOptions) -> OpsResult<Self> {
        let user = options
            .user
            .as_deref()
            .map(|user| resolve_user(repo, user))
            .transpose()?;
        Self::new(
            user,
            options.wave.clone(),
            options.stale.as_deref(),
            options.created_before.as_deref(),
            options.merged,
            options.include_open_prs,
        )
    }

    fn new(
        user: Option<String>,
        wave: Option<String>,
        stale: Option<&str>,
        created_before: Option<&str>,
        merged: bool,
        include_open_prs: bool,
    ) -> OpsResult<Self> {
        Ok(Self {
            user,
            wave,
            stale_days: stale.map(parse_duration_days).transpose()?,
            created_before: created_before.map(parse_date).transpose()?,
            merged,
            include_open_prs,
        })
    }
}

fn collect_branch_candidates(
    repo: &Path,
    filters: &BranchFilters,
) -> OpsResult<Vec<BranchCandidate>> {
    let default_branch = get_default_branch(repo)?;
    let _ = fetch_origin_prune(repo);
    let refs = remote_branch_refs(repo)?;
    let open_prs = if filters.include_open_prs {
        HashSet::new()
    } else {
        open_pr_branches(repo)
    };
    let current = current_branch(repo)?.unwrap_or_default();
    let branch_config = load_config(Some(repo))
        .ok()
        .flatten()
        .and_then(|config| config.branch_names);

    let mut candidates = Vec::new();
    for item in refs {
        if item.branch == "HEAD"
            || item.branch == default_branch
            || item.branch == "main"
            || item.branch == "master"
        {
            continue;
        }

        let parsed_wave = wave_name_from_branch(&item.branch, branch_config.as_ref());
        if let Some(user) = filters.user.as_deref() {
            if !matches_user(&item.author, user) {
                continue;
            }
        }
        if let Some(wave) = filters.wave.as_deref() {
            if !branch_matches_wave(&item.branch, parsed_wave.as_deref(), wave) {
                continue;
            }
        }
        if let Some(stale_days) = filters.stale_days {
            if age_days(item.committer_timestamp) < stale_days {
                continue;
            }
        }
        if let Some(cutoff) = filters.created_before {
            let created =
                first_unique_commit_date(repo, &item.branch)?.unwrap_or(item.committer_timestamp);
            let created_date = Utc
                .timestamp_opt(created, 0)
                .single()
                .ok_or_else(|| OpsError::Parse(format!("invalid commit timestamp: {created}")))?
                .date_naive();
            if created_date >= cutoff {
                continue;
            }
        }

        let merged = branch_is_merged(repo, &item.branch, &default_branch);
        if filters.merged && !merged {
            continue;
        }

        let open_pr = open_prs.contains(&item.branch);
        let (protected, protect_reason) = protected_reason(&item.branch, &current, open_pr);
        candidates.push(BranchCandidate {
            branch: item.branch,
            author: item.author,
            last_commit_date: format_date(item.committer_timestamp)?,
            age_days: age_days(item.committer_timestamp),
            wave: parsed_wave,
            merged,
            open_pr,
            protected,
            protect_reason,
        });
    }

    candidates.sort_by(|a, b| b.age_days.cmp(&a.age_days).then(a.branch.cmp(&b.branch)));
    Ok(candidates)
}

fn fetch_origin_prune(repo: &Path) -> OpsResult<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["fetch", "origin", "--prune"])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(OpsError::CommandFailed {
            command: "git fetch origin --prune".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn remote_branch_refs(repo: &Path) -> OpsResult<Vec<RemoteBranchRef>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "for-each-ref",
            "refs/remotes/origin/",
            "--format=%(refname:short)%00%(authorname)%00%(committerdate:unix)",
        ])
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "git for-each-ref refs/remotes/origin/".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut refs = Vec::new();
    for line in stdout.lines() {
        let mut parts = line.split('\0');
        let Some(ref_name) = parts.next() else {
            continue;
        };
        let Some(author) = parts.next() else { continue };
        let Some(timestamp) = parts.next() else {
            continue;
        };
        let Some(branch) = ref_name.strip_prefix("origin/") else {
            continue;
        };
        let committer_timestamp = timestamp
            .parse::<i64>()
            .map_err(|_| OpsError::Parse(format!("invalid commit timestamp: {timestamp}")))?;
        refs.push(RemoteBranchRef {
            branch: branch.to_string(),
            author: author.to_string(),
            committer_timestamp,
        });
    }
    Ok(refs)
}

fn open_pr_branches(repo: &Path) -> HashSet<String> {
    let output = Command::new("gh")
        .current_dir(repo)
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--json",
            "headRefName",
            "--limit",
            "1000",
            "--jq",
            ".[] | .headRefName",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .collect(),
        _ => HashSet::new(),
    }
}

fn branch_is_merged(repo: &Path, branch: &str, default_branch: &str) -> bool {
    let remote_branch = format!("origin/{branch}");
    let target = format!("origin/{default_branch}");
    let ancestor = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge-base", "--is-ancestor", &remote_branch, &target])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    ancestor || is_squash_merged(repo, &remote_branch, &target).unwrap_or(false)
}

fn first_unique_commit_date(repo: &Path, branch: &str) -> OpsResult<Option<i64>> {
    let default_branch = get_default_branch(repo)?;
    let range = format!("origin/{default_branch}..origin/{branch}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "--reverse", "--format=%ct", &range])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let first = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::to_string);
    first
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| OpsError::Parse(format!("invalid commit timestamp: {value}")))
        })
        .transpose()
}

fn protected_reason(branch: &str, current: &str, open_pr: bool) -> (bool, Option<String>) {
    let reason = if branch == "main" || branch == "master" {
        Some("base branch".to_string())
    } else if !current.is_empty() && branch == current {
        Some("current branch".to_string())
    } else if open_pr {
        Some("open PR".to_string())
    } else {
        None
    };
    (reason.is_some(), reason)
}

fn parse_duration_days(value: &str) -> OpsResult<i64> {
    let trimmed = value.trim();
    let split = trimmed
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, unit) = trimmed.split_at(split);
    if digits.is_empty() {
        return Err(OpsError::Parse(format!("invalid duration: {value}")));
    }
    let amount = digits
        .parse::<i64>()
        .map_err(|_| OpsError::Parse(format!("invalid duration: {value}")))?;
    let multiplier = match unit.trim() {
        "" | "d" | "day" | "days" => 1,
        "w" | "week" | "weeks" => 7,
        "m" | "mo" | "month" | "months" => 30,
        _ => return Err(OpsError::Parse(format!("invalid duration unit: {value}"))),
    };
    Ok(amount * multiplier)
}

fn parse_date(value: &str) -> OpsResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| OpsError::Parse(format!("invalid date: {value}")))
}

fn age_days(timestamp: i64) -> i64 {
    let now = Utc::now().timestamp();
    ((now - timestamp).max(0)) / 86_400
}

fn format_date(timestamp: i64) -> OpsResult<String> {
    let utc = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .ok_or_else(|| OpsError::Parse(format!("invalid commit timestamp: {timestamp}")))?;
    let local: DateTime<Local> = DateTime::from(utc);
    Ok(local.format("%Y-%m-%d").to_string())
}

fn resolve_user(repo: &Path, user: &str) -> OpsResult<String> {
    if user != "@me" {
        return Ok(user.to_string());
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.name"])
        .output()?;
    if output.status.success() {
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }

    Ok(std::env::var("USER").unwrap_or_else(|_| "user".to_string()))
}

fn matches_user(author: &str, user: &str) -> bool {
    author.eq_ignore_ascii_case(user)
}

fn branch_matches_wave(branch: &str, parsed_wave: Option<&str>, wave: &str) -> bool {
    if parsed_wave.is_some_and(|name| name == wave) {
        return true;
    }
    branch.split(['.', '/', '_', '-']).any(|part| part == wave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_accepts_days_weeks_months() {
        assert_eq!(parse_duration_days("30d").unwrap(), 30);
        assert_eq!(parse_duration_days("2w").unwrap(), 14);
        assert_eq!(parse_duration_days("1month").unwrap(), 30);
    }

    #[test]
    fn branch_wave_match_uses_schema_or_segments() {
        assert!(branch_matches_wave(
            "jack.redesign.20260401_1200",
            Some("redesign"),
            "redesign"
        ));
        assert!(branch_matches_wave(
            "feature/redesign-cleanup",
            None,
            "redesign"
        ));
        assert!(!branch_matches_wave("feature/predesign", None, "redesign"));
    }
}
