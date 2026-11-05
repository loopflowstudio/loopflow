//! Old command spellings, kept for one release after the LOO-593 namespace
//! alignment moved each command to its API name. This module is deleted
//! whole in the release after 0.15, together with its one call in
//! `navigation::normalize_args`; nothing else names an old spelling.
//!
//! alias until the release after 0.15: LOO-593 alignment

/// One old spelling: the command path as typed (`path`, which ends where the
/// current tree stops recognizing it), words after it that belong to the old
/// spelling and are dropped (`words`, such as `--stop`), and the canonical
/// command path that replaces both.
pub struct Moved {
    pub path: &'static [&'static str],
    pub words: &'static [&'static str],
    pub canonical: &'static [&'static str],
}

const fn moved(
    path: &'static [&'static str],
    words: &'static [&'static str],
    canonical: &'static [&'static str],
) -> Moved {
    Moved {
        path,
        words,
        canonical,
    }
}

pub const MOVED: &[Moved] = &[
    moved(&["ops"], &[], &[]),
    moved(&["config", "show"], &[], &["config", "settings"]),
    moved(&["task", "edit"], &[], &["task", "update"]),
    moved(&["wave", "add"], &[], &["wave", "create"]),
    moved(&["machine", "id"], &[], &["machine", "self"]),
    moved(&["history", "usage"], &[], &["session", "usage"]),
    moved(&["history", "retention"], &[], &["session", "retention"]),
    moved(&["self", "doctor"], &[], &["machine", "doctor"]),
    moved(&["config", "planning"], &[], &["repository", "planning"]),
    moved(
        &["config", "planning"],
        &["status"],
        &["repository", "planning"],
    ),
    moved(
        &["config", "repository"],
        &["--path"],
        &["repository", "path"],
    ),
];

/// The old spelling `path` (the command words resolved) and `rest` (every
/// word typed after them) spell, the one naming the most old words.
pub fn find(path: &[String], rest: &[String]) -> Option<&'static Moved> {
    MOVED
        .iter()
        .filter(|entry| {
            entry.path.len() == path.len()
                && entry.path.iter().zip(path).all(|(old, typed)| old == typed)
                && entry
                    .words
                    .iter()
                    .all(|word| rest.iter().any(|typed| typed == word))
        })
        .max_by_key(|entry| entry.words.len())
}

/// `path` (command words only, as `lf help` takes them) with its longest old
/// prefix replaced by the canonical path.
pub fn rewrite(path: &[String]) -> Vec<String> {
    (1..=path.len())
        .rev()
        .find_map(|split| {
            find(&path[..split], &path[split..]).map(|entry| {
                let mut words: Vec<String> = entry
                    .canonical
                    .iter()
                    .map(|word| word.to_string())
                    .collect();
                let mut dropped = entry.words.to_vec();
                for word in &path[split..] {
                    match dropped.iter().position(|old| old == word) {
                        Some(at) => {
                            dropped.remove(at);
                        }
                        None => words.push(word.clone()),
                    }
                }
                words
            })
        })
        .unwrap_or_else(|| path.to_vec())
}
