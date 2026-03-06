use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::builtins::get_builtin_ops_prompt;
use crate::engine::config::load_config_or_default;
use crate::engine::git::{commit, current_branch, is_clean, push, push_with_upstream, stage_all};

use crate::ops::error::{OpsError, OpsResult};
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

    let message = if let Some(message) = options
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
    {
        message.to_string()
    } else {
        progress.status("Generating commit message...");
        let generated = generate_commit_message(repo);
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

fn generate_commit_message(repo: &Path) -> OpsResult<CommitMessage> {
    let template = get_builtin_ops_prompt("commit_message")
        .ok_or_else(|| OpsError::Message("builtin commit_message prompt not found".to_string()))?;

    let diff = staged_diff(repo)?;
    let diff = truncate_chars(&diff, 20_000);

    let prompt = format!(
        "{template}\n\n## Staged diff\n```diff\n{diff}\n```\n\n\
         Return exactly one JSON object with this schema:\n\
         {{\"title\":\"...\",\"body\":\"...\"}}\n\
         No markdown fences. No explanation."
    );

    let config = load_config_or_default(Some(repo));
    let agent = config
        .agent
        .clone()
        .unwrap_or_else(|| "claude:haiku".to_string());

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
        format_commit_message(&options.task, &options.flow_parents, Some("generated title"))
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
