use std::path::Path;

use serde::Deserialize;

use loopflow_engine::agent::{launch_agent, LaunchConfig};
use loopflow_engine::config::load_config_or_default;
use loopflow_engine::prompt::count_tokens;

use crate::error::{OpsError, OpsResult};

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

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct MessagePayload {
    title: String,
    body: String,
}

pub fn generate_commit_message(repo: &Path) -> OpsResult<Message> {
    let diff = get_staged_diff(repo)?;
    let prompt = build_message_prompt(diff.as_deref(), COMMIT_MESSAGE_PROMPT);
    generate_message(repo, &prompt)
}

pub fn generate_pr_message(repo: &Path) -> OpsResult<Message> {
    let diff = get_branch_diff(repo)?;
    let prompt = build_message_prompt(diff.as_deref(), PR_MESSAGE_PROMPT);
    generate_message(repo, &prompt)
}

pub fn generate_pr_message_from_diff(repo: &Path, diff: &str) -> OpsResult<Message> {
    let prompt = build_message_prompt(Some(diff), PR_MESSAGE_PROMPT);
    generate_message(repo, &prompt)
}

fn generate_message(repo: &Path, prompt: &str) -> OpsResult<Message> {
    let config = load_config_or_default(Some(repo));
    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
        context_file: None,
        ..Default::default()
    };
    let prompt = format!(
        "{}\n\nReturn JSON with keys: title, body. No extra text.",
        prompt
    );
    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    parse_message_output(&result.stdout)
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
    if let Some(payload) = extract_json_payload(trimmed) {
        return Ok(Message {
            title: payload.title,
            body: payload.body,
        });
    }

    let lines = trimmed
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(OpsError::Parse("empty message output".to_string()));
    }
    let title = lines[0].to_string();
    let body = if lines.len() > 1 {
        lines[1..].join("\n")
    } else {
        String::new()
    };
    Ok(Message { title, body })
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

fn get_staged_diff(repo: &Path) -> OpsResult<Option<String>> {
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg("--cached")
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

fn get_branch_diff(repo: &Path) -> OpsResult<Option<String>> {
    let base_ref = crate::pr::default_base_ref(repo)?;
    let output = std::process::Command::new("git")
        .arg("diff")
        .arg(format!("{}...HEAD", base_ref))
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
    use super::{parse_message_output, Message};

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
    fn parse_message_output_fallback() {
        let output = "title line\nbody line";
        let message = parse_message_output(output).expect("parse");
        assert_eq!(message.title, "title line");
        assert_eq!(message.body, "body line");
    }
}
