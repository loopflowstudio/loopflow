//! Session names, author slugs, and branch-safe sanitization.
//!
//! Branch/worktree identity itself lives in [`crate::engine::identity`]. This
//! module only supplies the raw pieces it composes.

use crate::engine::error::GitError;
use std::path::Path;
use std::process::Command;

const SESSION_TITLE_MAX_CHARS: usize = 80;

pub(crate) fn generated_session_title(
    context: Option<&crate::trace::PreparedTurnContext>,
    skill: Option<&str>,
    task: Option<&str>,
    cwd: &Path,
) -> String {
    let skill = skill.map(crate::engine::definition_name::definition_key);
    let purpose = skill.as_deref().filter(|skill| {
        !matches!(
            *skill,
            "session"
                | "operate"
                | "repo-session"
                | "wave-session"
                | "task-session"
                | "repo-operate"
                | "wave-operate"
                | "task-operate"
        )
    });
    let title = [
        context.and_then(session_request),
        purpose,
        task,
        cwd.file_name().and_then(|name| name.to_str()),
    ]
    .into_iter()
    .flatten()
    .find_map(request_title)
    .unwrap_or_else(|| "Session".to_string());
    title
}

pub(crate) fn request_title(source: &str) -> Option<String> {
    let title = source
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|word| {
            !word.is_empty()
                && !matches!(
                    word.to_ascii_lowercase().as_str(),
                    "please" | "the" | "a" | "an"
                )
        })
        .take(3)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(SESSION_TITLE_MAX_CHARS)
        .collect::<String>();
    (!title.is_empty()).then_some(title)
}

fn session_request(context: &crate::trace::PreparedTurnContext) -> Option<&str> {
    use crate::trace::ContextAssetKind;

    for channel in context.system.iter().chain(std::iter::once(&context.task)) {
        for asset in channel
            .assets
            .iter()
            .filter(|asset| asset.kind == ContextAssetKind::UserMessage)
        {
            if let Some(request) = channel
                .text
                .get(asset.byte_start as usize..asset.byte_end as usize)
            {
                return Some(request);
            }
        }
    }
    // Library callers have unassembled, separate system/task prompts.
    let task = &context.task;
    (task.text != crate::engine::prompt::INITIAL_TURN_PROMPT
        && task
            .assets
            .iter()
            .all(|asset| asset.kind == ContextAssetKind::Assembly))
    .then_some(task.text.as_str())
}

/// One trimmed, non-empty line of at most `SESSION_TITLE_MAX_CHARS` characters.
pub(crate) fn validate_session_title(title: &str) -> std::io::Result<&str> {
    let title = title.trim();
    if title.is_empty() || title.contains(['\n', '\r']) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Session name must be one non-empty line",
        ));
    }
    if title.chars().count() > SESSION_TITLE_MAX_CHARS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Session name must be at most {SESSION_TITLE_MAX_CHARS} characters"),
        ));
    }
    Ok(title)
}

/// Reduce an arbitrary string to a branch-safe slug: lowercase alphanumerics,
/// `-`, `_`, `.`, with runs of anything else collapsed to a single `-`.
pub fn sanitize_for_branch(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        let lowered = ch.to_ascii_lowercase();
        let keep =
            lowered.is_ascii_alphanumeric() || lowered == '-' || lowered == '_' || lowered == '.';
        if keep {
            out.push(lowered);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for ch in trimmed.chars() {
        if ch == '-' {
            if !prev_dash {
                collapsed.push(ch);
            }
            prev_dash = true;
        } else {
            collapsed.push(ch);
            prev_dash = false;
        }
    }
    if collapsed.is_empty() {
        "user".to_string()
    } else {
        collapsed
    }
}

/// The git author as a branch-safe slug, for the remote-branch author prefix.
/// Falls back to `$USER`, then `"user"`.
pub fn git_user(repo: &Path) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "user.name"])
        .output()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Ok(sanitize_for_branch(&name));
        }
    }
    let fallback = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    Ok(sanitize_for_branch(&fallback))
}

#[cfg(test)]
mod tests {
    use super::sanitize_for_branch;

    #[test]
    fn generated_titles_use_work_and_never_the_system_trigger() {
        let cwd = std::path::Path::new("/repo/terminal-titles");
        let context = crate::trace::PreparedTurnContext::from_prompts(
            "# Loopflow operating guide",
            crate::engine::prompt::INITIAL_TURN_PROMPT,
        );
        assert_eq!(
            super::generated_session_title(
                Some(&context),
                Some("demo"),
                Some("Terminal titles"),
                cwd
            ),
            "demo"
        );
        assert_eq!(
            super::generated_session_title(
                Some(&context),
                Some("task/session"),
                Some("Repair cmux titles"),
                cwd
            ),
            "Repair cmux titles"
        );
        assert_eq!(
            super::generated_session_title(Some(&context), Some("wave/operate"), None, cwd),
            "terminal-titles"
        );
        let context = crate::trace::PreparedTurnContext::from_prompts(
            "# Loopflow operating guide",
            &format!("{} other words", "λ".repeat(100)),
        );
        let title = super::generated_session_title(Some(&context), None, None, cwd);
        assert_eq!(title.chars().count(), super::SESSION_TITLE_MAX_CHARS);
        assert!(super::validate_session_title(&title).is_ok());
    }

    #[test]
    fn sanitize_for_branch_cleans_input() {
        assert_eq!(sanitize_for_branch("Jack Heart!!!"), "jack-heart");
    }

    #[test]
    fn sanitize_removes_special_chars() {
        assert_eq!(sanitize_for_branch("feat/my thing!"), "feat-my-thing");
    }

    #[test]
    fn sanitize_collapses_hyphens() {
        assert_eq!(sanitize_for_branch("a---b"), "a-b");
    }

    #[test]
    fn sanitize_trims_leading_trailing() {
        assert_eq!(sanitize_for_branch("-foo-"), "foo");
    }
}
