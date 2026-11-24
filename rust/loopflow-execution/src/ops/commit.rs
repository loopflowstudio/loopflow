use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::agent::{run_agent, AgentConfig, ProcessConfig};
use crate::config::load_config_or_default;
use crate::git::{
    commit, current_branch, git_stdout, git_stdout_inheriting, is_clean, push, push_with_upstream,
    stage_all,
};
use crate::graph::load_skill;

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::prompt::change::{Change, ChangeRange};

#[derive(Debug, Clone)]
pub struct CommitOptions {
    pub add: bool,
    pub push: bool,
    pub create_draft_pr: bool,
    pub task: String,
    pub sources: Vec<String>,
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
            sources: Vec::new(),
            message: None,
            agent: None,
        }
    }
}

/// Commit an explicit document selection using an independent index. The real
/// index retains unrelated staged work, and scratch remains on disk.
pub fn commit_selected(repo: &Path, paths: &[String], message: Option<&str>) -> OpsResult<()> {
    let message =
        message.ok_or_else(|| OpsError::Message("selected-path commits require -m".into()))?;
    let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
    restart_landed_persistent(repo)?;
    let directory = tempfile::tempdir()?;
    let index = directory.path().join("index");
    let inherit_index = |command: &mut Command| {
        command.env("GIT_INDEX_FILE", &index);
    };
    let run = |args: &[&str]| git_stdout_inheriting(repo, args, &inherit_index);
    run(&["read-tree", "HEAD"])?;
    let selected = paths
        .iter()
        .map(|path| format!(":(literal){path}"))
        .collect::<Vec<_>>();
    if !paths.is_empty() {
        let mut args = vec!["add", "-A", "--"];
        args.extend(selected.iter().map(String::as_str));
        run(&args)?;
    }
    if run(&["diff", "--cached", "--name-only"])?.trim().is_empty() {
        return Err(OpsError::Message(
            "Nothing selected to commit; edit the selected files or select changed paths.".into(),
        ));
    }
    commit(repo, message, &inherit_index)?;
    let mut reset = vec!["reset", "-q", "HEAD", "--"];
    reset.extend(selected.iter().map(String::as_str));
    git_stdout(repo, &reset).map_err(|error| {
        OpsError::Message(format!("commit saved; index refresh failed: {error}"))
    })?;
    Ok(())
}

/// Whenever a persistent checkout is about to commit or publish, drop history
/// its merged PRs already delivered. Nothing has to observe the merge itself.
pub(crate) fn restart_landed_persistent(repo: &Path) -> OpsResult<()> {
    if crate::git::worktrees::is_persistent_worktree(repo)?
        && crate::ops::sync::restart_landed_branch(repo)?
    {
        eprintln!("Earlier commits have merged; restarted this branch from the default branch.");
    }
    Ok(())
}

/// Run the ordinary commit path, carrying caller-owned capabilities into Git mutations.
/// Agent generation, draft PR creation and Task merge revocation have separate launch owners.
pub fn commit_workflow(
    repo: &Path,
    options: &CommitOptions,
    progress: &impl Progress,
    inherit_git: &impl Fn(&mut Command),
) -> OpsResult<bool> {
    restart_landed_persistent(repo)?;
    if is_clean(repo)? {
        progress.status("Nothing to commit");
        if options.push {
            progress.status("Pushing...");
            push_with_upstream_if_needed(repo, inherit_git)?;
            if options.create_draft_pr {
                ensure_draft_pr(repo, progress)?;
            }
        }
        return Ok(false);
    }

    if options.add {
        progress.status("Staging changes...");
        stage_all(repo, inherit_git)?;
    }

    if git_stdout(repo, &["diff", "--cached", "--name-only"])?
        .trim()
        .is_empty()
    {
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
        match generate_commit_message(repo, MessageAgent::Chosen(options.agent.as_deref())) {
            Ok(message) => message.text(),
            Err(error) => {
                progress.error(&error.to_string());
                format_commit_message(&options.task, &options.sources)
            }
        }
    };

    progress.status("Committing...");
    commit(repo, &message, inherit_git)?;

    if options.push {
        progress.status("Pushing...");
        push_with_upstream_if_needed(repo, inherit_git)?;
        if options.create_draft_pr {
            ensure_draft_pr(repo, progress)?;
        }
    }

    Ok(true)
}

fn format_commit_message(task: &str, sources: &[String]) -> String {
    if sources.is_empty() {
        format!("lf {task}")
    } else {
        format!("lf {} {task}", sources.join(" "))
    }
}

#[derive(Debug, Deserialize)]
struct CommitMessage {
    title: String,
    body: String,
}

impl CommitMessage {
    fn text(&self) -> String {
        match self.body.trim() {
            "" => self.title.trim().to_string(),
            body => format!("{}\n\n{body}", self.title.trim()),
        }
    }
}

/// Commit what a skill left as `lf <step>: <what changed>` with a body, and
/// push the branch unless it is the default branch, including commits the
/// skill made itself. The `commit-message` skill writes the message on its
/// own configured agents, and on `agent` (the run's `-a`) only when it has
/// none; without an answer the title is `lf <step>: checkpoint`.
///
/// # Errors
/// Returns the Git failure when staging, committing or pushing fails.
pub fn commit_step(
    repo: &Path,
    step: &str,
    agent: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<bool> {
    stage_all(repo, &|_| {})?;
    let staged = !git_stdout(repo, &["diff", "--cached", "--name-only"])?
        .trim()
        .is_empty();
    let written = if staged {
        progress.status("Writing the commit message...");
        generate_commit_message(repo, MessageAgent::Fallback(agent))
            .inspect_err(|error| progress.error(&error.to_string()))
            .ok()
    } else {
        None
    };
    commit_workflow(
        repo,
        &CommitOptions {
            push: off_default_branch(repo)?,
            message: Some(step_commit_message(step, written.as_ref())),
            ..CommitOptions::for_task(step)
        },
        progress,
        &|_| {},
    )
}

/// Whether a finished skill step left work for [`commit_step`]: edits to
/// commit, or commits a branch other than the default hasn't pushed. A
/// checkout Git can't read has nothing to commit.
pub fn step_commit_pending(repo: &Path) -> bool {
    let unpushed = || -> OpsResult<bool> {
        if !off_default_branch(repo)? {
            return Ok(false);
        }
        Ok(match crate::git::rev_parse(repo, "@{upstream}") {
            Ok(upstream) => upstream != crate::git::rev_parse(repo, "HEAD")?,
            Err(_) => true,
        })
    };
    !is_clean(repo).unwrap_or(true) || unpushed().unwrap_or(false)
}

pub(crate) fn off_default_branch(repo: &Path) -> OpsResult<bool> {
    let default_branch = crate::git::get_default_branch(repo)?;
    Ok(current_branch(repo)?.is_some_and(|branch| branch != default_branch))
}

fn step_commit_message(step: &str, written: Option<&CommitMessage>) -> String {
    match written.filter(|message| !message.title.trim().is_empty()) {
        Some(message) => format!("lf {step}: {}", message.text()),
        None => format!("lf {step}: checkpoint"),
    }
}

/// A hung provider must not hold the Graph or the skill run that asked.
const COMMIT_MESSAGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Where an agent named on the command line stands against the message
/// skill's own preference.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MessageAgent<'a> {
    /// `-a`: the person chose the agent for this message.
    Chosen(Option<&'a str>),
    /// A step commit: the run's agent writes the message only when the skill
    /// resolves none of its own.
    Fallback(Option<&'a str>),
}

fn generate_commit_message(repo: &Path, agent: MessageAgent<'_>) -> OpsResult<CommitMessage> {
    let skill = load_skill("commit-message", repo)
        .map_err(|err| OpsError::Message(format!("commit-message skill not found: {err}")))?;
    let template = skill
        .content
        .clone()
        .ok_or_else(|| OpsError::Message("commit-message skill has no content".to_string()))?;

    let change = Change::read(repo, ChangeRange::Staged)?.prompt_section();

    let prompt = format!(
        "{template}\n\n## Change\n{change}\n\
         Return exactly one JSON object with this schema:\n\
         {{\"title\":\"...\",\"body\":\"...\"}}\n\
         No markdown fences. No explanation."
    );

    let process = ProcessConfig {
        stream: false,
        timeout: Some(COMMIT_MESSAGE_TIMEOUT),
        ..Default::default()
    };
    let launch = message_launch(repo, &skill, agent, prompt)?;
    let failure = match run_agent(&launch, &process, crate::harness::LaunchKind::Batch) {
        Ok(result) if result.exit_code == 0 => {
            let combined = format!("{}\n{}", result.stdout, result.stderr);
            if let Some(message) = parse_commit_message(&combined) {
                return Ok(message);
            }
            "no commit message in the reply".to_owned()
        }
        Ok(result) => format!("exit {}: {}", result.exit_code, result.stderr.trim()),
        Err(error) => error.to_string(),
    };
    Err(OpsError::Message(format!(
        "commit message generation failed ({failure})"
    )))
}

/// The launch that writes a message for `skill`. The skill's own `agent`,
/// `default_agent` and `agents` lead, so a cheap model can be chosen for
/// `commit-message` or `pr-message` alone, and the rest of the order is where
/// the launch moves when a provider refuses it.
pub(crate) fn message_launch(
    repo: &Path,
    skill: &crate::graph::Skill,
    agent: MessageAgent<'_>,
    prompt: String,
) -> OpsResult<AgentConfig> {
    let config = load_config_or_default(Some(repo));
    let mut config = crate::config::settings::resolve(&config, Some(skill))
        .map(|resolved| resolved.config)
        .unwrap_or(config);
    match agent {
        MessageAgent::Chosen(Some(agent)) => config.agent = Some(agent.to_owned()),
        MessageAgent::Fallback(Some(agent)) if config.agent().is_err() => {
            config.agent = Some(agent.to_owned())
        }
        MessageAgent::Chosen(None) | MessageAgent::Fallback(_) => {}
    }
    let (agent, fallback_agents) = config.resolve_agents()?;
    Ok(AgentConfig {
        chrome: config.chrome,
        codex_service_tier: config.codex.service_tier,
        task_prompt: prompt,
        cwd: Some(repo.to_path_buf()),
        yolo: config.yolo,
        fallback_agents,
        ..AgentConfig::new(agent)
    })
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

pub(crate) fn push_with_upstream_if_needed(
    repo: &Path,
    inherit_git: &impl Fn(&mut Command),
) -> OpsResult<()> {
    // Every ordinary Loopflow branch push shares the Task settlement fence.
    // After a commit, a changed HEAD clears a head-pinned merge request (and
    // revokes Auto remotely) before Git can expose the new head. Same-head
    // publication remains a no-op and preserves the request.
    let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
    crate::ops::task::clear_task_pr_merge_before_head_mutation(repo, false, inherit_git)?;
    push_with_upstream_if_needed_locked(repo, inherit_git)
}

fn push_with_upstream_if_needed_locked(
    repo: &Path,
    inherit_git: &impl Fn(&mut Command),
) -> OpsResult<()> {
    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".into()))?;
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if let Ok(previous) = crate::git::rev_parse(repo, &remote_ref) {
        if !crate::git::is_ancestor(repo, &previous, "HEAD")? {
            let base = crate::git::integration_base(repo)?;
            if crate::git::is_ancestor(repo, &base, "HEAD")?
                && crate::ops::sync::landed_at(repo, &base, &previous)?
            {
                let reference = format!("refs/heads/{branch}");
                let remote = git_stdout(repo, &["ls-remote", "--heads", "origin", &reference])?;
                if remote.split_whitespace().next() == Some(previous.as_str()) {
                    // Replace only the exact head delivered to a base retained
                    // in HEAD. Divergent rewrites and concurrent pushes survive.
                    let lease = format!("--force-with-lease={reference}:{previous}");
                    return git_stdout_inheriting(
                        repo,
                        &["push", &lease, "-u", "origin", &reference],
                        inherit_git,
                    )
                    .map(|_| ())
                    .map_err(OpsError::from);
                }
            }
        }
    }
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("--symbolic-full-name")
        .arg("@{upstream}")
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        push(repo, false, inherit_git)?;
        return Ok(());
    }

    push_with_upstream(repo, "origin", &branch, inherit_git)?;
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

#[cfg(test)]
mod tests {
    use super::{step_commit_message, CommitMessage};

    #[test]
    fn step_commits_name_the_skill_before_what_changed() {
        let written = CommitMessage {
            title: "Count retries per Task".into(),
            body: "The board read attempts from the wrong row.".into(),
        };
        assert_eq!(
            step_commit_message("implement", Some(&written)),
            "lf implement: Count retries per Task\n\nThe board read attempts from the wrong row."
        );
        let untitled = CommitMessage {
            title: " ".into(),
            body: "orphan body".into(),
        };
        assert_eq!(
            step_commit_message("implement", Some(&untitled)),
            "lf implement: checkpoint"
        );
        assert_eq!(
            step_commit_message("reflect", None),
            "lf reflect: checkpoint"
        );
    }
}
