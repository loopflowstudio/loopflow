use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::{
    commit, current_branch, is_clean, push, push_with_upstream, rev_parse, stage_all,
};
use crate::engine::load_skill;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::{NullProgress, Progress};
use crate::ops::trace::{MockResponses, Tracer};

#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub add: bool,
    pub push: bool,
    pub create_draft_pr: bool,
    pub task: String,
    pub flow_parents: Vec<String>,
    pub message: Option<String>,
    pub agent: Option<String>,
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
            agent: None,
        }
    }
}

/// Stage, commit, and push a Task worktree so its state survives this
/// machine. Runs off the async worker thread because commit_workflow drives
/// its own runtime for the push settlement fence.
pub(crate) async fn checkpoint_task_worktree(
    worktree: std::path::PathBuf,
    task_identifier: String,
    message: String,
) -> anyhow::Result<()> {
    let outcome = tokio::task::spawn_blocking(move || {
        let options = CommitOptions {
            add: true,
            push: true,
            create_draft_pr: false,
            task: task_identifier,
            flow_parents: Vec::new(),
            message: Some(message),
            agent: None,
        };
        commit_workflow(&worktree, &options, &NullProgress).map(|_| ())
    })
    .await
    .map_err(|join_error| anyhow::anyhow!("Task worktree checkpoint panicked: {join_error}"))?;
    outcome.map_err(anyhow::Error::from)
}

pub(crate) fn checkpoint_task_restart(worktree: &Path, task_identifier: &str) -> OpsResult<String> {
    let _mutation = crate::ops::task::lock_task_pr_mutation(worktree)?;
    if !is_clean(worktree)? {
        stage_all(worktree)?;
        verify_restart_preimage(worktree)?;
    }
    let options = CommitOptions {
        add: false,
        push: false,
        create_draft_pr: false,
        task: task_identifier.to_string(),
        flow_parents: Vec::new(),
        message: Some(format!("checkpoint: restart {task_identifier}")),
        agent: None,
    };
    commit_workflow(worktree, &options, &NullProgress)?;
    crate::ops::task::clear_task_pr_merge_before_head_mutation(worktree, false)?;
    push_with_upstream_if_needed_locked(worktree)?;
    rev_parse(worktree, "HEAD").map_err(OpsError::Git)
}

fn verify_restart_preimage(worktree: &Path) -> OpsResult<()> {
    let unstaged = std::process::Command::new("git")
        .args(["diff", "--quiet", "--no-ext-diff"])
        .current_dir(worktree)
        .status()?;
    let untracked = std::process::Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(worktree)
        .output()?;
    if !unstaged.success() || !untracked.stdout.is_empty() {
        return Err(OpsError::Message(
            "Task worktree changed while restart prepared its checkpoint; review the staged snapshot and retry"
                .to_string(),
        ));
    }
    Ok(())
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

    let message = if let Some(message) = options
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
    {
        message.to_string()
    } else {
        progress.status("Generating commit message...");
        let generated = generate_commit_message(repo, options.agent.as_deref());
        format_commit_message(
            &options.task,
            &options.flow_parents,
            generated.as_ref().map(|m| m.title.as_str()).ok(),
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

fn has_staged_changes(repo: &Path) -> OpsResult<bool> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .current_dir(repo)
        .status()?;
    Ok(!output.success())
}

fn format_commit_message(task: &str, flow_parents: &[String], title: Option<&str>) -> String {
    let prefix = if flow_parents.is_empty() {
        format!("lf {task}")
    } else {
        format!("lf {} {task}", flow_parents.join(" "))
    };

    match title {
        Some(title) if !title.is_empty() => format!("{prefix}: {title}"),
        _ => prefix,
    }
}

#[derive(Debug, Deserialize)]
struct CommitMessage {
    title: String,
    #[allow(dead_code)]
    body: String,
}

fn generate_commit_message(repo: &Path, agent_override: Option<&str>) -> OpsResult<CommitMessage> {
    let template = load_skill("commit-message", repo)
        .map_err(|err| OpsError::Message(format!("commit-message skill not found: {err}")))?
        .content
        .ok_or_else(|| OpsError::Message("commit-message skill has no content".to_string()))?;

    let diff = staged_diff(repo)?;
    let diff = truncate_chars(&diff, 20_000);

    let prompt = format!(
        "{template}\n\n## Staged diff\n```diff\n{diff}\n```\n\n\
         Return exactly one JSON object with this schema:\n\
         {{\"title\":\"...\",\"body\":\"...\"}}\n\
         No markdown fences. No explanation."
    );

    let config = load_config_or_default(Some(repo));
    let agent = agent_override.unwrap_or_else(|| config.agent()).to_string();

    let launch = AgentConfig {
        task_prompt: prompt,
        agent: Some(agent),
        cwd: Some(repo.to_path_buf()),
        skip_permissions: true,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: false,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = launch_agent(&launch, &process, &capabilities)
        .map_err(|err| OpsError::Message(format!("commit message generation failed: {err}")))?;
    if result.exit_code != 0 {
        return Err(OpsError::Message(format!(
            "commit message generation failed (exit {}): {}",
            result.exit_code,
            result.stderr.trim()
        )));
    }

    let combined = format!("{}\n{}", result.stdout, result.stderr);
    parse_commit_message(&combined)
        .ok_or_else(|| OpsError::Message("failed to parse commit message from agent".to_string()))
}

fn staged_diff(repo: &Path) -> OpsResult<String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(repo)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_commit_message(raw: &str) -> Option<CommitMessage> {
    let trimmed = raw.trim();
    // Try direct JSON parse
    if let Ok(msg) = serde_json::from_str::<CommitMessage>(trimmed) {
        return Some(msg);
    }
    // Try fenced JSON
    if let Some(start) = trimmed.find("```json") {
        let rest = &trimmed[start + "```json".len()..];
        if let Some(end) = rest.find("```") {
            if let Ok(msg) = serde_json::from_str::<CommitMessage>(rest[..end].trim()) {
                return Some(msg);
            }
        }
    }
    // Try extracting JSON object
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str::<CommitMessage>(&trimmed[start..=end]).ok()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .nth(max_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    format!("{}\n\n[diff truncated]", &text[..end])
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

pub(crate) fn push_with_upstream_if_needed(repo: &Path) -> OpsResult<()> {
    // Every ordinary Loopflow branch push shares the Task settlement fence.
    // After a commit, a changed HEAD clears a head-pinned merge request (and
    // revokes Auto remotely) before Git can expose the new head. Same-head
    // publication remains a no-op and preserves the request.
    let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
    crate::ops::task::clear_task_pr_merge_before_head_mutation(repo, false)?;
    push_with_upstream_if_needed_locked(repo)
}

fn push_with_upstream_if_needed_locked(repo: &Path) -> OpsResult<()> {
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

pub(crate) fn verify_remote_branch_head(
    repo: &Path,
    branch: &str,
    expected_head: &str,
) -> OpsResult<()> {
    let reference = format!("refs/heads/{branch}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-remote", "--heads", "origin", &reference])
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git ls-remote --heads origin {reference}"),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    let remote_head = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    if remote_head != expected_head {
        return Err(OpsError::Message(format!(
            "origin/{branch} is {remote_head}, expected the pushed head {expected_head}"
        )));
    }
    Ok(())
}

/// Traced version of commit_workflow for parity testing.
/// Returns JSON trace instead of executing operations.
pub fn commit_workflow_traced(options: &CommitOptions) -> String {
    let mut tracer = Tracer::new(
        "commit",
        json!({
            "add": options.add,
            "push": options.push,
            "create_draft_pr": options.create_draft_pr,
            "has_message": options.message.is_some()
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

    // Resolve commit message: explicit or generated.
    let message = if let Some(message) = options
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
    {
        message.to_string()
    } else {
        tracer.trace("commit:generate_message");
        format_commit_message(
            &options.task,
            &options.flow_parents,
            Some("generated title"),
        )
    };

    // Commit
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
