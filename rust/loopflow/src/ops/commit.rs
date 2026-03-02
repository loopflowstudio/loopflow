use std::path::Path;

use crate::engine::git::{commit, current_branch, is_clean, push, push_with_upstream, stage_all};
use serde_json::json;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::messages::generate_commit_message;
use crate::ops::progress::Progress;
use crate::ops::trace::{MockResponses, Tracer};

#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub add: bool,
    pub push: bool,
    pub create_draft_pr: bool,
    pub task: String,
    pub flow_parents: Vec<String>,
    pub message: Option<String>,
}

impl CommitOptions {
    pub fn for_task(task: impl Into<String>) -> Self {
        Self {
            add: false,
            push: false,
            create_draft_pr: false,
            task: task.into(),
            flow_parents: Vec::new(),
            message: None,
        }
    }
}

pub fn commit_workflow(
    repo: &Path,
    options: &CommitOptions,
    progress: &impl Progress,
) -> OpsResult<bool> {
    if is_clean(repo)? {
        progress.status("Nothing to commit");
        if options.push {
            push_with_upstream_if_needed(repo)?;
            if options.create_draft_pr {
                ensure_draft_pr(repo, progress)?;
            }
        }
        return Ok(false);
    }

    if options.add {
        progress.status("Staging changes...");
        stage_all(repo)?;
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
        format_commit_message(
            &options.task,
            &options.flow_parents,
            &generated.title,
            &generated.body,
        )
    };

    progress.status("Committing...");
    commit(repo, &message)?;

    if options.push {
        push_with_upstream_if_needed(repo)?;
        if options.create_draft_pr {
            ensure_draft_pr(repo, progress)?;
        }
    }

    Ok(true)
}

fn format_commit_message(task: &str, flow_parents: &[String], title: &str, body: &str) -> String {
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
    if let Err(_err) = push(repo, false) {
        if let Err(force_err) = push(repo, true) {
            return Err(OpsError::Git(force_err));
        }
    }
    Ok(())
}

fn ensure_draft_pr(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    if !crate::ops::pr::gh_available() {
        return Ok(());
    }

    if crate::ops::pr::pr_exists_for_current_branch(repo)? {
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

    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    push_with_upstream(repo, "origin", &branch)?;
    Ok(())
}

pub(crate) fn push_with_upstream_or_error(repo: &Path) -> OpsResult<()> {
    push_with_upstream_if_needed(repo)
}

/// Traced version of commit_workflow for parity testing.
/// Returns JSON trace instead of executing operations.
pub fn commit_workflow_traced(options: &CommitOptions) -> String {
    let mut tracer = Tracer::new(
        "commit",
        json!({
            "add": options.add,
            "push": options.push,
            "create_draft_pr": options.create_draft_pr
        }),
    );

    // Check if repo is dirty
    tracer.trace_result("git:status", MockResponses::git_status());
    let is_dirty = MockResponses::git_status() == "dirty";

    if !is_dirty {
        return tracer.to_json();
    }

    // Stage changes
    if options.add {
        tracer.trace("git:add_all");
    }

    // Check if anything staged
    tracer.trace_result("git:diff_cached", MockResponses::git_diff_cached());
    let has_staged = MockResponses::git_diff_cached() == "has_changes";

    if !has_staged {
        return tracer.to_json();
    }

    // Agent generates commit message
    let (title, _body) = MockResponses::agent_commit_message();
    tracer.trace_args(
        "agent:run",
        json!({
            "prompt_hash": "mock_hash",
            "model": "claude:opus"
        }),
    );

    // Commit
    let message = format!("lf test: {}", title);
    tracer.trace_args("git:commit", json!({"message": message}));

    // Push
    if options.push {
        let has_upstream = MockResponses::git_has_upstream();
        tracer.trace_result("git:has_upstream", &has_upstream.to_string());

        if has_upstream {
            tracer.trace("git:push");

            let pr_exists = MockResponses::gh_pr_exists();
            tracer.trace_result("gh:pr_exists", &pr_exists.to_string());

            if !pr_exists {
                tracer.trace("gh:pr_create_draft");
            }
        }
    }

    tracer.to_json()
}
