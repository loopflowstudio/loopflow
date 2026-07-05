//! Channels: named streams under one wave's family.
//!
//! A CHANNEL is a named stream — journal + thread + subscribability. The
//! wave's own channel is its name (`goals`), journal under the origin repo
//! (`.lf/journal/waves/<wave>/`). A work line's channel is the ownership
//! name — exactly the worktree basename minus the repo prefix
//! (`goals.148e0e02`) — and its journal lives IN THAT WORKTREE
//! (`<worktree>/.lf/journal/waves/<channel>/journal.jsonl`): it travels with
//! the branch and dies with it. Channels are conversations, not records — at
//! land the mind curates the distilled story up (parent channel + wave
//! memory) and the raw work-line journal dies with the worktree. FLAGGED,
//! unbuilt: if lived experience misses raw records,
//! `~/.lf/journal/<repo>/<worktree>` is the pre-named persistent archive.
//!
//! Names are topics, dots are the tree: `goals.148e0e02` is inside `goals`'s
//! family; subscription is by name or prefix. The family head's server holds
//! the pen for every child channel — single-writer per journal file, all
//! pens in one process. Child channels have NO mind: they are pure streams
//! (no `MindState`, no memory — a work line's notes are files; MEMORY.md is
//! wave identity).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tokio::sync::broadcast;

use crate::lfd::conversations::turns::ChatTurn;
use crate::wave::journal::{
    fold_thread, journal_path, Attribution, EventKind, Journal, MessageId, MessageOp,
};

// Family MEMBERSHIP lives in one place: `runtime::channel_role` (it compares
// against the sanitized wave name — the form channel names actually carry).
// This module keeps only the name mechanics: prefix matching, head split,
// path derivation.

/// Whether `channel` matches a subtree `prefix`: the prefix itself or any
/// dot-descendant of it.
pub fn matches_prefix(channel: &str, prefix: &str) -> bool {
    channel == prefix
        || channel
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with('.'))
}

/// The family head a channel name belongs to — the wave, its first dot
/// segment (`goals.148e0e02` → `goals`; `goals` → `goals`).
pub fn family_head(channel: &str) -> &str {
    channel.split('.').next().unwrap_or(channel)
}

/// A child channel's worktree — the ownership naming inverted: channel
/// `goals.148e0e02` under origin `/src/loopflow` lives at the sibling
/// `/src/loopflow.goals.148e0e02`.
pub fn child_worktree_path(origin: &Path, channel: &str) -> PathBuf {
    let repo_name = origin
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    origin
        .parent()
        .unwrap_or(origin)
        .join(format!("{repo_name}.{channel}"))
}

/// A child channel's journal, inside its own worktree.
pub fn child_journal_path(origin: &Path, channel: &str) -> PathBuf {
    journal_path(&child_worktree_path(origin, channel), channel)
}

/// One live frame from a child channel, tagged with its name so a family
/// subscription can tell the streams apart (the primary channel's frames ride
/// untagged — absent means the wave's own channel).
#[derive(Debug, Clone)]
pub struct ChannelFrame {
    pub channel: String,
    pub turn: std::sync::Arc<ChatTurn>,
    /// The tagged wire JSON (turn + `"channel"` key), serialized once at the
    /// send site so N subscribers share one serialization.
    pub json: std::sync::Arc<str>,
}

/// A child channel's turn as `/events` wire JSON: the `Turn` object plus one
/// extra key, `"channel"`. Additive — the primary channel's frames stay
/// untagged, so a family of one is byte-identical to the pre-family wire.
/// Shared by the live send site (once per frame) and the replay path.
pub fn tagged_turn_json(channel: &str, turn: &ChatTurn) -> String {
    let mut value = serde_json::to_value(turn).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(map) = &mut value {
        map.insert(
            "channel".to_string(),
            serde_json::Value::String(channel.to_string()),
        );
    }
    value.to_string()
}

/// A materialized child channel: the pen over its journal plus the thread
/// fold. No mind, no memory, no inbox — deliveries land in the journal and
/// the thread, and broadcast on the family bus; nothing consumes them here
/// (consumption markers never cross journals, and a mindless channel has no
/// consumer).
#[derive(Debug)]
pub struct ChildChannel {
    name: String,
    /// The worktree the journal lives in — checked before every append so a
    /// landed/deleted worktree ends the channel instead of resurrecting its
    /// directory tree.
    worktree: PathBuf,
    inner: Mutex<ChildInner>,
}

#[derive(Debug)]
struct ChildInner {
    journal: Journal,
    thread: Vec<ChatTurn>,
}

impl ChildChannel {
    /// Open the channel's journal (creating it if the worktree exists) and
    /// fold its thread. `None` when the worktree is gone — the channel ended
    /// with its branch.
    pub fn open(origin: &Path, name: &str) -> anyhow::Result<Option<Self>> {
        let worktree = child_worktree_path(origin, name);
        if !worktree.is_dir() {
            return Ok(None);
        }
        let (journal, events) = Journal::open(&journal_path(&worktree, name))?;
        let fold = fold_thread(&events);
        let mut thread = fold.turns;
        thread.extend(fold.open);
        Ok(Some(Self {
            name: name.to_string(),
            worktree,
            inner: Mutex::new(ChildInner { journal, thread }),
        }))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the channel's worktree still exists. A vanished worktree ends
    /// the channel: reads serve nothing, writes refuse.
    pub fn alive(&self) -> bool {
        self.worktree.is_dir()
    }

    /// The thread as folded so far (finalized order; children have no open
    /// mind turns).
    pub fn thread_snapshot(&self) -> Vec<ChatTurn> {
        self.inner
            .lock()
            .expect("child channel lock poisoned")
            .thread
            .clone()
    }

    /// Deliver one message to this channel: journal the `UserMessage`, commit
    /// the turn, and broadcast it tagged on the family bus. A bare interrupt
    /// is a no-op (`None`) — there is no resident to interrupt; `steer`
    /// degrades to a plain message the same way a mindless queue would treat
    /// it. Refuses (`Err`) when the worktree vanished between polls.
    pub fn deliver(
        &self,
        family_tx: &broadcast::Sender<ChannelFrame>,
        op: MessageOp,
        text: String,
        from: Option<Attribution>,
    ) -> anyhow::Result<Option<ChatTurn>> {
        if op == MessageOp::Interrupt && text.trim().is_empty() {
            return Ok(None);
        }
        if !self.alive() {
            anyhow::bail!(
                "channel '{}' ended: its worktree is gone ({})",
                self.name,
                self.worktree.display()
            );
        }
        let mut inner = self.inner.lock().expect("child channel lock poisoned");
        let event = inner.journal.append(|seq| EventKind::UserMessage {
            id: MessageId(format!("msg-{seq}")),
            op,
            text: text.clone(),
            from: from.clone(),
        });
        let mut turn = ChatTurn::user(format!("turn-{}", event.seq), text);
        turn.created_at = event.at_rfc3339();
        turn.from = from.map(|from| from.label);
        inner.thread.push(turn.clone());
        // A send error just means no live subscribers.
        let _ = family_tx.send(ChannelFrame {
            channel: self.name.clone(),
            json: tagged_turn_json(&self.name, &turn).into(),
            turn: std::sync::Arc::new(turn.clone()),
        });
        Ok(Some(turn))
    }
}

/// Discover the channels already on disk under `origin`'s family for `wave`:
/// sibling worktrees named `<repo>.<wave>.…` that carry a channel journal.
/// Tolerates a worktree vanishing mid-scan — a dead branch's channel just
/// isn't there.
pub fn scan_child_channels(origin: &Path, wave: &str) -> Vec<String> {
    let repo_name = origin
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let Some(parent) = origin.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let prefix = format!("{repo_name}.{wave}.");
    let mut channels: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .filter_map(|dir| {
            dir.strip_prefix(&prefix)
                .map(|rest| format!("{wave}.{rest}"))
        })
        .filter(|channel| child_journal_path(origin, channel).is_file())
        .collect();
    channels.sort();
    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_naming_rules() {
        assert_eq!(family_head("goals.148e0e02"), "goals");
        assert_eq!(family_head("goals"), "goals");
        assert!(matches_prefix("goals", "goals"));
        assert!(matches_prefix("goals.148e0e02", "goals"));
        assert!(matches_prefix("goals.a.b", "goals.a"));
        assert!(!matches_prefix("goals.ab", "goals.a"));
        assert!(!matches_prefix("goalsmith", "goals"));
        assert!(!matches_prefix("concerto", "goals"));
    }

    #[test]
    fn child_paths_follow_the_ownership_naming() {
        let origin = Path::new("/src/loopflow");
        assert_eq!(
            child_worktree_path(origin, "goals.148e0e02"),
            Path::new("/src/loopflow.goals.148e0e02")
        );
        assert_eq!(
            child_journal_path(origin, "goals.148e0e02"),
            Path::new(
                "/src/loopflow.goals.148e0e02/.lf/journal/waves/goals.148e0e02/journal.jsonl"
            )
        );
    }

    #[test]
    fn open_returns_none_for_a_missing_worktree() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        assert!(ChildChannel::open(&origin, "ship.dead")
            .expect("open tolerates absence")
            .is_none());
    }

    #[test]
    fn deliver_journals_commits_and_broadcasts_tagged() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(child_worktree_path(&origin, "ship.abc")).unwrap();
        let channel = ChildChannel::open(&origin, "ship.abc")
            .expect("open")
            .expect("worktree exists");
        let (tx, mut rx) = broadcast::channel(8);

        let turn = channel
            .deliver(
                &tx,
                MessageOp::Say,
                "landed".into(),
                Some(Attribution {
                    session_id: None,
                    label: "worker".into(),
                }),
            )
            .expect("deliver")
            .expect("a say appends");
        assert_eq!(turn.text, "landed");
        assert_eq!(turn.from.as_deref(), Some("worker"));
        let frame = rx.try_recv().expect("tagged frame");
        assert_eq!(frame.channel, "ship.abc");
        assert_eq!(frame.turn.id, turn.id);

        // A bare interrupt is a no-op on a mindless channel.
        assert!(channel
            .deliver(&tx, MessageOp::Interrupt, "  ".into(), None)
            .expect("noop")
            .is_none());

        // The journal is durable: a reopened channel folds the same thread.
        drop(channel);
        let reopened = ChildChannel::open(&origin, "ship.abc")
            .expect("reopen")
            .expect("still there");
        let thread = reopened.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "landed");
    }

    #[test]
    fn scan_finds_only_journaled_family_worktrees() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        // Two real channels, one bare worktree (no journal), one foreign dir.
        for name in ["ship.a", "ship.b"] {
            std::fs::create_dir_all(child_worktree_path(&origin, name)).unwrap();
            Journal::open(&child_journal_path(&origin, name)).expect("init journal");
        }
        std::fs::create_dir_all(child_worktree_path(&origin, "ship.bare")).unwrap();
        std::fs::create_dir_all(tmp.path().join("repo.other.c")).unwrap();

        assert_eq!(
            scan_child_channels(&origin, "ship"),
            vec!["ship.a", "ship.b"]
        );
    }
}
