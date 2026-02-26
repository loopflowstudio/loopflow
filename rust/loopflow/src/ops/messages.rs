use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::current_branch;
use crate::engine::naming::wave_name_from_branch;
use crate::engine::prompt::count_tokens;
use crate::engine::worktrees::wave_name_from_worktree;

use crate::ops::error::{OpsError, OpsResult};

const COMMIT_MESSAGE_PROMPT: &str = r#"Generate a commit message for the staged changes.

Review the diff and write a concise commit message.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Output format

Return a structured response with:
- **title**: lowercase, with optional area prefix (e.g. `llm_http: add structured output`)
- **body**: brief explanation if needed, otherwise empty string

## Title style

Titles are lowercase and concise. Use an area prefix when changes are focused on a specific module or feature area.

Examples:
- `llm_http: add structured output for pr messages`
- `pr workflow: add -a flag to commit and push`
- `fix typo in readme`

## Body style

Keep it brief - one sentence or a few bullets if the change needs explanation. Empty string is fine for self-explanatory changes.

Most commits don't need a body. Only add one if the "why" isn't obvious from the title.
"#;

const PR_MESSAGE_PROMPT: &str = r#"Generate a PR title and body for the changes on this branch.

Review the diff against the PR base and summarize what changed and why.

Do not ask questions. If anything is unclear, make the best assumption and proceed.

## Output format

Return a structured response with:
- **title**: lowercase, with optional area prefix (e.g. `llm_http: add structured output`)
- **body**: markdown with headers, code blocks for commands, and bullet lists

## Title style

Titles are lowercase and concise. Use an area prefix when changes are focused on a specific module or feature area. The area can be new or existing.

Examples:
- `llm_http: add structured output for pr messages`
- `pr workflow: add -a flag to commit and push`
- `fix worktree cleanup on branch delete`

## Body style

Use markdown headers to organize the body. Open with a "Usage" or "Try it" section showing commands in code blocks. Then explain what changed.

Structure:
1. **Usage section** (header + code block) - how to try it, run it, or see it in action
2. **Summary** - one paragraph explaining what this PR does and why
3. **Changes** (optional) - bullet list of notable changes if helpful

Keep it medium length. Stay high-level; don't enumerate every file.
"#;

const DIFF_BUDGET: usize = 120_000;
const TITLE_MAX_CHARS: usize = 100;
const NON_TRIVIAL_DIFF_CHANGE_LINES: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Commit,
    PullRequest { non_trivial_diff: bool },
}

impl MessageKind {
    fn requires_body(self) -> bool {
        match self {
            Self::Commit => false,
            Self::PullRequest { non_trivial_diff } => non_trivial_diff,
        }
    }

    fn log_label(self) -> &'static str {
        match self {
            Self::Commit => "commit-message",
            Self::PullRequest { .. } => "pr-message",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub title: String,
    pub body: String,
}

impl Message {
    pub fn validate(&self, require_body: bool) -> OpsResult<()> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err(OpsError::Parse("message title is empty".to_string()));
        }
        if title.chars().count() > TITLE_MAX_CHARS {
            return Err(OpsError::Parse(format!(
                "message title exceeds {TITLE_MAX_CHARS} characters"
            )));
        }

        let lower_title = title.to_ascii_lowercase();
        if lower_title.contains("http://") || lower_title.contains("https://") {
            return Err(OpsError::Parse(
                "message title must not contain URLs".to_string(),
            ));
        }

        if require_body && self.body.trim().is_empty() {
            return Err(OpsError::Parse(
                "message body is required for non-trivial diffs".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct MessagePayload {
    title: String,
    body: String,
}

pub fn generate_commit_message(repo: &Path) -> OpsResult<Message> {
    let diff = get_staged_diff(repo)?;
    let prompt = build_message_prompt(diff.as_deref(), COMMIT_MESSAGE_PROMPT);
    generate_message(repo, &prompt, MessageKind::Commit)
}

pub fn generate_pr_message(repo: &Path, wave: Option<&str>) -> OpsResult<Message> {
    let diff = get_branch_diff(repo)?;
    generate_pr_message_with_diff(repo, diff.as_deref(), wave)
}

pub fn generate_pr_message_from_diff(
    repo: &Path,
    diff: &str,
    wave: Option<&str>,
) -> OpsResult<Message> {
    generate_pr_message_with_diff(repo, Some(diff), wave)
}

fn generate_pr_message_with_diff(
    repo: &Path,
    diff: Option<&str>,
    wave: Option<&str>,
) -> OpsResult<Message> {
    let wave = wave.and_then(normalize_wave_name);
    let non_trivial_diff = diff.is_some_and(is_non_trivial_diff);
    let task = if let Some(wave_name) = wave.as_deref() {
        format!(
            "{PR_MESSAGE_PROMPT}\n\n## Topic\n\nAlways use `{wave_name}` as the area prefix in the title. Do not invent a different topic."
        )
    } else {
        PR_MESSAGE_PROMPT.to_string()
    };
    let prompt = build_message_prompt(diff, &task);
    let mut message =
        generate_message(repo, &prompt, MessageKind::PullRequest { non_trivial_diff })?;
    if let Some(wave_name) = wave.as_deref() {
        message.title = enforce_wave_title_prefix(&message.title, wave_name);
    }
    message.validate(non_trivial_diff)?;
    Ok(message)
}

/// Resolve the wave name for PR message generation.
///
/// Priority: explicit wave name > worktree directory name > branch name component.
pub fn resolve_wave_name(repo: &Path, explicit: Option<&str>) -> Option<String> {
    if let Some(name) = explicit.and_then(normalize_wave_name) {
        return Some(name);
    }
    if let Some(name) = wave_name_from_worktree(repo).and_then(|name| normalize_wave_name(&name)) {
        return Some(name);
    }
    if let Ok(Some(branch)) = current_branch(repo) {
        let config = load_config_or_default(Some(repo));
        return wave_name_from_branch(&branch, config.branch_names.as_ref())
            .and_then(|name| normalize_wave_name(&name));
    }
    None
}

fn normalize_wave_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed
        .strip_prefix("wave/")
        .unwrap_or(trimmed)
        .trim_matches('/');
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn enforce_wave_title_prefix(title: &str, wave_name: &str) -> String {
    let canonical_prefix = format!("{wave_name}:");
    let prefixed_wave = format!("wave/{wave_name}");
    let trimmed = title.trim();

    if let Some((prefix, rest)) = trimmed.split_once(':') {
        let prefix = prefix.trim();
        let rest = rest.trim();
        if prefix == wave_name || prefix == prefixed_wave {
            return if rest.is_empty() {
                canonical_prefix
            } else {
                format!("{canonical_prefix} {rest}")
            };
        }
        if rest.is_empty() {
            return format!("{canonical_prefix} {prefix}");
        }
        return format!("{canonical_prefix} {rest}");
    }

    if trimmed.is_empty() {
        canonical_prefix
    } else {
        format!("{canonical_prefix} {trimmed}")
    }
}

fn generate_message(repo: &Path, prompt: &str, kind: MessageKind) -> OpsResult<Message> {
    let config = load_config_or_default(Some(repo));
    let launch = AgentConfig {
        task_prompt: format!(
            "{}\n\nReturn JSON with keys: title, body. No extra text.",
            prompt
        ),
        model: Some(config.agent_model.clone()),
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
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    let log_path = write_message_output_log(repo, kind.log_label(), &result.stdout, &result.stderr);
    if result.exit_code != 0 {
        return Err(append_log_hint_to_error(
            OpsError::AgentFailed(result.stderr),
            log_path.as_deref(),
        ));
    }
    let message = parse_message_output(&result.stdout)
        .map_err(|err| append_log_hint_to_error(err, log_path.as_deref()))?;
    message
        .validate(kind.requires_body())
        .map_err(|err| append_log_hint_to_error(err, log_path.as_deref()))?;
    Ok(message)
}

fn build_message_prompt(diff: Option<&str>, task_prompt: &str) -> String {
    let mut parts = Vec::new();
    if let Some(diff) = diff {
        parts.push(format!(
            "<lf:diff>\n{}\n</lf:diff>",
            limit_diff(diff, DIFF_BUDGET)
        ));
    }
    parts.push(format!("<lf:task>\n{}\n</lf:task>", task_prompt));
    parts.join("\n\n")
}

fn limit_diff(diff: &str, budget: usize) -> String {
    let tokens = count_tokens(diff);
    if tokens <= budget {
        return diff.to_string();
    }
    let char_limit = (budget as f64 * 3.5) as usize;
    let mut out = diff.chars().take(char_limit).collect::<String>();
    out.push_str("\n\n[...diff truncated to fit context limit...]");
    out
}

fn parse_message_output(output: &str) -> OpsResult<Message> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Err(OpsError::Parse("empty message output".to_string()));
    }
    let payload = extract_json_payload(trimmed)
        .ok_or_else(|| OpsError::Parse("expected JSON with title and body".to_string()))?;
    Ok(Message {
        title: payload.title,
        body: payload.body,
    })
}

fn extract_json_payload(text: &str) -> Option<MessagePayload> {
    if let Ok(payload) = serde_json::from_str::<MessagePayload>(text) {
        return Some(payload);
    }
    if let Some(payload) = extract_fenced_json(text) {
        return Some(payload);
    }
    extract_inline_json(text)
}

fn extract_fenced_json(text: &str) -> Option<MessagePayload> {
    let fence = text.find("```json")?;
    let rest = &text[fence + 7..];
    let end = rest.find("```")?;
    let candidate = rest[..end].trim();
    serde_json::from_str::<MessagePayload>(candidate).ok()
}

fn extract_inline_json(text: &str) -> Option<MessagePayload> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start >= end {
        return None;
    }
    let candidate = text[start..=end].trim();
    serde_json::from_str::<MessagePayload>(candidate).ok()
}

fn is_non_trivial_diff(diff: &str) -> bool {
    diff.lines()
        .filter(|line| {
            (line.starts_with('+') && !line.starts_with("+++"))
                || (line.starts_with('-') && !line.starts_with("---"))
        })
        .count()
        >= NON_TRIVIAL_DIFF_CHANGE_LINES
}

fn write_message_output_log(
    repo: &Path,
    label: &str,
    stdout: &str,
    stderr: &str,
) -> Option<PathBuf> {
    let log_dir = repo.join(".lf").join("logs");
    std::fs::create_dir_all(&log_dir).ok()?;
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
    let filename = format!("{timestamp}-{}-{label}.log", std::process::id());
    let path = log_dir.join(filename);
    let contents = format!(
        "message generation output\n\n=== stdout ===\n{stdout}\n\n=== stderr ===\n{stderr}\n"
    );
    std::fs::write(&path, contents).ok()?;
    Some(path)
}

fn append_log_hint_to_error(err: OpsError, log_path: Option<&Path>) -> OpsError {
    let Some(log_path) = log_path else {
        return err;
    };
    let hint = format!(" (raw output logged at {})", log_path.display());
    match err {
        OpsError::Parse(message) => OpsError::Parse(format!("{message}{hint}")),
        OpsError::Message(message) => OpsError::Message(format!("{message}{hint}")),
        OpsError::AgentFailed(message) => OpsError::AgentFailed(format!("{message}{hint}")),
        other => other,
    }
}

fn get_staged_diff(repo: &Path) -> OpsResult<Option<String>> {
    run_git_diff(repo, &["diff", "--cached"])
}

fn get_branch_diff(repo: &Path) -> OpsResult<Option<String>> {
    let main_repo =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let base_branch = crate::engine::git::get_default_branch(&main_repo)?;
    let range = format!("origin/{base_branch}...HEAD");
    run_git_diff(repo, &["diff", &range])
}

fn run_git_diff(repo: &Path, args: &[&str]) -> OpsResult<Option<String>> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let diff = String::from_utf8_lossy(&output.stdout).to_string();
    if diff.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(diff))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        enforce_wave_title_prefix, is_non_trivial_diff, normalize_wave_name, parse_message_output,
        resolve_wave_name, write_message_output_log, Message, NON_TRIVIAL_DIFF_CHANGE_LINES,
        TITLE_MAX_CHARS,
    };
    use crate::ops::error::OpsError;

    #[test]
    fn parse_message_output_json() {
        let output = r#"{"title":"hello","body":"world"}"#;
        let message = parse_message_output(output).expect("parse");
        assert_eq!(
            message,
            Message {
                title: "hello".to_string(),
                body: "world".to_string()
            }
        );
    }

    #[test]
    fn parse_message_output_rejects_non_json() {
        let output = "title line\nbody line";
        let error = parse_message_output(output).expect_err("parse should fail");
        assert!(matches!(error, OpsError::Parse(message) if message.contains("expected JSON")));
    }

    #[test]
    fn validate_rejects_url_in_title() {
        let message = Message {
            title: "updated pr #396: https://example.com".to_string(),
            body: "body".to_string(),
        };
        let error = message.validate(false).expect_err("validation should fail");
        assert!(
            matches!(error, OpsError::Parse(message) if message.contains("must not contain URLs"))
        );
    }

    #[test]
    fn validate_rejects_title_over_character_limit() {
        let message = Message {
            title: "a".repeat(TITLE_MAX_CHARS + 1),
            body: "body".to_string(),
        };
        let error = message.validate(false).expect_err("validation should fail");
        assert!(matches!(error, OpsError::Parse(message) if message.contains("exceeds")));
    }

    #[test]
    fn validate_requires_body_for_non_trivial_diff() {
        let message = Message {
            title: "good title".to_string(),
            body: "   ".to_string(),
        };
        let error = message.validate(true).expect_err("validation should fail");
        assert!(matches!(error, OpsError::Parse(message) if message.contains("required")));
    }

    #[test]
    fn validate_allows_empty_body_for_trivial_diff() {
        let message = Message {
            title: "good title".to_string(),
            body: String::new(),
        };
        message.validate(false).expect("validation should pass");
    }

    #[test]
    fn non_trivial_diff_detects_threshold_crossing() {
        let mut diff = String::new();
        for index in 0..NON_TRIVIAL_DIFF_CHANGE_LINES {
            diff.push_str(&format!("+line {index}\n"));
        }
        assert!(is_non_trivial_diff(&diff));
    }

    #[test]
    fn non_trivial_diff_ignores_trivial_changes() {
        let diff = "+line 1\n+line 2\n";
        assert!(!is_non_trivial_diff(diff));
    }

    #[test]
    fn write_message_output_log_records_stdout_and_stderr() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = write_message_output_log(tempdir.path(), "pr-message", "hello", "warning")
            .expect("log path");
        let content = std::fs::read_to_string(path).expect("read log");
        assert!(content.contains("hello"));
        assert!(content.contains("warning"));
    }

    #[test]
    fn normalize_wave_name_strips_wave_prefix() {
        assert_eq!(
            normalize_wave_name("wave/mobile"),
            Some("mobile".to_string())
        );
        assert_eq!(normalize_wave_name("mobile"), Some("mobile".to_string()));
        assert_eq!(normalize_wave_name(""), None);
    }

    #[test]
    fn enforce_wave_title_prefix_rewrites_wave_path_prefix() {
        let title = enforce_wave_title_prefix("wave/mobile: polish terminal ux", "mobile");
        assert_eq!(title, "mobile: polish terminal ux");
    }

    #[test]
    fn enforce_wave_title_prefix_replaces_mismatched_prefix() {
        let title = enforce_wave_title_prefix("ux: polish terminal ux", "mobile");
        assert_eq!(title, "mobile: polish terminal ux");
    }

    #[test]
    fn enforce_wave_title_prefix_adds_missing_prefix() {
        let title = enforce_wave_title_prefix("polish terminal ux", "mobile");
        assert_eq!(title, "mobile: polish terminal ux");
    }

    #[test]
    fn resolve_wave_name_normalizes_explicit_prefix() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let wave = resolve_wave_name(tempdir.path(), Some("wave/mobile"));
        assert_eq!(wave, Some("mobile".to_string()));
    }
}
