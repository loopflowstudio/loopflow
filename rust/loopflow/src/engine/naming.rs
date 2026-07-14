//! Name primitives shared by the identity layer: author slug and branch-safe
//! sanitization.
//!
//! Branch/worktree identity itself lives in [`crate::engine::identity`]. This
//! module only supplies the raw pieces it composes.

use crate::engine::error::GitError;
use std::path::Path;
use std::process::Command;

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
    use super::*;

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
