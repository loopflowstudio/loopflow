use std::path::Path;

use loopflow_engine::git::{commit, current_branch, is_clean, push, push_with_upstream, stage_all};

use crate::error::{OpsError, OpsResult};
use crate::messages::generate_commit_message;
use crate::progress::Progress;

#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub add: bool,
    pub lint: bool,
    pub push: bool,
    pub task: String,
    pub flow_parents: Vec<String>,
    pub message: Option<String>,
}

pub fn commit_workflow(repo: &Path, options: &CommitOptions, progress: &impl Progress) -> OpsResult<bool> {
    if is_clean(repo)? {
        progress.status("Nothing to commit");
        if options.push {
            push_with_upstream_if_needed(repo)?;
            ensure_draft_pr(repo, progress)?;
        }
        return Ok(false);
    }

    if options.add {
        progress.status("Staging changes...");
        stage_all(repo)?;
    }

    if options.lint {
        if !crate::lint::ensure_lint_passes(repo, progress)? {
            return Err(OpsError::LintFailed);
        }
    }

    if !has_staged_changes(repo)? {
        progress.status("Nothing staged to commit");
        return Ok(false);
    }

    let message = if let Some(message) = &options.message {
        message.to_string()
    } else {
        progress.status("Generating commit message...");
        let generated = generate_commit_message(repo)?;
        format_commit_message(&options.task, &options.flow_parents, &generated.title, &generated.body)
    };

    progress.status("Committing...");
    commit(repo, &message)?;

    if options.push {
        push_with_upstream_if_needed(repo)?;
        ensure_draft_pr(repo, progress)?;
    }

    Ok(true)
}

fn format_commit_message(
    task: &str,
    flow_parents: &[String],
    title: &str,
    body: &str,
) -> String {
    let prefix = if flow_parents.is_empty() {
        format!("lf {task}")
    } else {
        format!("lf {} {task}", flow_parents.join(" "))
    };

    if body.trim().is_empty() {
        format!("{prefix}: {title}")
    } else {
        format!("{prefix}: {title}\n\n{body}")
    }
}

fn has_staged_changes(repo: &Path) -> OpsResult<bool> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .current_dir(repo)
        .status()?;
    Ok(!output.success())
}

fn push_with_fallback(repo: &Path) -> OpsResult<()> {
    if let Err(err) = push(repo, false) {
        let force_result = push(repo, true);
        if force_result.is_err() {
            return Err(OpsError::Git(err));
        }
    }
    Ok(())
}

fn ensure_draft_pr(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    if !crate::pr::gh_available() {
        return Ok(());
    }

    if crate::pr::pr_exists_for_current_branch(repo)? {
        return Ok(());
    }

    progress.status("Creating draft PR...");
    let output = std::process::Command::new("gh")
        .arg("pr")
        .arg("create")
        .arg("--draft")
        .arg("--fill")
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    progress.error(&format!(
        "Draft PR creation failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ));
    Ok(())
}

fn push_with_upstream_if_needed(repo: &Path) -> OpsResult<()> {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("--symbolic-full-name")
        .arg("@{upstream}")
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        push_with_fallback(repo)?;
        return Ok(());
    }

    let branch = current_branch(repo)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    push_with_upstream(repo, "origin", &branch)?;
    Ok(())
}

pub(crate) fn push_with_upstream_or_error(repo: &Path) -> OpsResult<()> {
    push_with_upstream_if_needed(repo)
}
