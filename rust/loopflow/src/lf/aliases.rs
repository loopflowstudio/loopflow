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
    moved(&["session", "history"], &["usage"], &["session", "usage"]),
    moved(
        &["session", "history"],
        &["retention"],
        &["session", "retention"],
    ),
    moved(&["self", "doctor"], &[], &["machine", "doctor"]),
    moved(
        &["machine", "doctor"],
        &["--planning"],
        &["machine", "doctor", "--plan"],
    ),
    moved(&["config", "planning"], &[], &["repo", "plan"]),
    moved(&["config", "planning"], &["status"], &["repo", "plan"]),
    moved(&["config", "repository"], &[], &["config", "repo"]),
    moved(&["config", "repository"], &["--path"], &["repo", "path"]),
    moved(&["repository"], &[], &["repo"]),
    moved(&["repository"], &["planning"], &["repo", "plan"]),
    moved(&["repository", "planning"], &[], &["repo", "plan"]),
    moved(&["desktop", "task"], &[], &["task", "show"]),
    moved(&["desktop", "done"], &[], &["task", "list", "--closed"]),
    moved(
        &["task", "list"],
        &["--done"],
        &["task", "list", "--closed"],
    ),
    moved(&["task", "diff"], &["--files"], &["task", "changes"]),
    moved(&["ship"], &["--stop"], &["task", "hold"]),
    moved(&["desktop", "sessions"], &[], &["session", "list"]),
    moved(&["desktop", "wave"], &[], &["wave", "plan"]),
    moved(&["wave", "status"], &["--sync"], &["wave", "sync"]),
    moved(&["wave", "bind-project"], &[], &["wave", "plan", "bind"]),
    moved(&["wave", "connect"], &[], &["wave", "plan", "connect"]),
    moved(&["wave", "reteam"], &[], &["wave", "plan", "reteam"]),
    moved(&["desktop", "plan"], &[], &["plan", "list"]),
    moved(&["wave", "planning"], &[], &["wave", "plan"]),
    moved(&["planning"], &[], &["repo", "plan"]),
    moved(&["planning"], &["list"], &["plan", "list"]),
    moved(&["planning"], &["status"], &["repo", "plan"]),
    moved(&["planning"], &["key"], &["repo", "plan", "key"]),
    moved(&["planning"], &["journal"], &["repo", "plan", "journal"]),
    moved(&["desktop", "plans"], &[], &["desktop", "view", "list"]),
    moved(
        &["desktop", "view"],
        &["--plan"],
        &["desktop", "view", "show"],
    ),
    moved(
        &["desktop", "query"],
        &[],
        &["desktop", "dashboard", "query"],
    ),
    moved(&["desktop", "pin"], &[], &["desktop", "dashboard", "pin"]),
    moved(
        &["desktop", "unpin"],
        &[],
        &["desktop", "dashboard", "unpin"],
    ),
    moved(
        &["desktop", "rename"],
        &[],
        &["desktop", "dashboard", "rename"],
    ),
    moved(&["monitor"], &[], &["process", "list"]),
    moved(&["monitor"], &["ps"], &["process", "list"]),
    moved(&["monitor"], &["top"], &["process", "top"]),
    moved(&["monitor"], &["prune"], &["process", "prune"]),
    moved(&["ps"], &[], &["process", "list"]),
    moved(&["graph", "show"], &["--processes"], &["process", "show"]),
    moved(&["comment"], &[], &["task", "steer"]),
    moved(&["task", "comment"], &["--steer"], &["task", "steer"]),
    moved(&["task", "comment"], &[], &["task", "report"]),
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
