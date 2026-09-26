//! Name primitives: author slug, branch-safe sanitization, and the
//! `magical-musical` word pair that names raw Sessions.
//!
//! Branch/worktree identity itself lives in [`crate::engine::identity`]. This
//! module only supplies the raw pieces it composes.

use crate::engine::error::GitError;
use std::path::Path;
use std::process::Command;

const MAGICAL: &[&str] = &[
    "aurora", "cascade", "crystal", "drift", "echo", "ember", "fern", "flume", "frost", "glade",
    "grove", "haze", "ivy", "jade", "luna", "mist", "nova", "opal", "petal", "prism", "rain",
    "ripple", "sage", "shade", "spark", "star", "stone", "storm", "tide", "vale", "wave", "wisp",
    "wren", "zephyr",
];

const MUSICAL: &[&str] = &[
    "allegro", "aria", "ballad", "cadence", "canon", "chord", "coda", "duet", "forte", "fugue",
    "harmony", "hymn", "lilt", "lyric", "melody", "motif", "opus", "prelude", "refrain", "rondo",
    "sonata", "tempo", "trill", "tune", "verse", "waltz",
];

/// A stable `magical-musical` pair, e.g. `aurora-fugue`, derived from `seed`.
/// The same seed always yields the same pair, so readers need not persist it.
pub fn word_pair(seed: &str) -> String {
    // FNV-1a: stable across Rust releases, unlike the std hasher.
    let hash = seed.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let magical = MAGICAL[(hash % MAGICAL.len() as u64) as usize];
    let musical = MUSICAL[((hash >> 32) % MUSICAL.len() as u64) as usize];
    format!("{magical}-{musical}")
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
    fn word_pairs_are_stable_and_vary_by_seed() {
        let pair = word_pair("run_1");
        assert_eq!(pair, word_pair("run_1"));
        let (magical, musical) = pair.split_once('-').expect("two words");
        assert!(MAGICAL.contains(&magical) && MUSICAL.contains(&musical));
        let distinct = (0..20)
            .map(|index| word_pair(&format!("run_{index}")))
            .collect::<std::collections::HashSet<_>>();
        assert!(distinct.len() > 10);
    }

    #[test]
    fn sanitize_trims_leading_trailing() {
        assert_eq!(sanitize_for_branch("-foo-"), "foo");
    }
}
