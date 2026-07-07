//! Name primitives shared by the identity layer: author slug, word pairs for
//! de-collision, timestamps, and branch-safe sanitization.
//!
//! Branch/worktree identity itself lives in [`crate::engine::identity`]. This
//! module only supplies the raw pieces it composes.

use crate::engine::error::GitError;
use chrono::Local;
use rand::prelude::IndexedRandom;
use rand::Rng;
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

fn generate_word_pair_with_rng<R: Rng + ?Sized>(rng: &mut R) -> String {
    let magical = MAGICAL.choose(rng).copied().unwrap_or("wisp");
    let musical = MUSICAL.choose(rng).copied().unwrap_or("forte");
    format!("{magical}-{musical}")
}

/// A `magical-musical` pair, e.g. `aurora-fugue`, for de-colliding names.
pub fn generate_word_pair() -> String {
    let mut rng = rand::rng();
    generate_word_pair_with_rng(&mut rng)
}

/// The current local time as `YYYYMMDD_HHMM`.
pub fn generate_timestamp() -> String {
    Local::now().format("%Y%m%d_%H%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

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

    #[test]
    fn generate_timestamp_has_expected_shape() {
        let ts = generate_timestamp();
        let (date, time) = ts.split_once('_').expect("underscore-separated");
        assert_eq!(date.len(), 8);
        assert_eq!(time.len(), 4);
    }

    #[test]
    fn word_pairs_are_two_words() {
        let mut rng = StdRng::seed_from_u64(7);
        let pair = generate_word_pair_with_rng(&mut rng);
        let parts: Vec<&str> = pair.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts.iter().all(|p| !p.is_empty()));
    }

    #[test]
    fn word_pairs_vary_with_different_seeds() {
        let a = generate_word_pair_with_rng(&mut StdRng::seed_from_u64(1));
        let b = generate_word_pair_with_rng(&mut StdRng::seed_from_u64(2));
        assert_ne!(a, b);
    }
}
