//! Shared filesystem watch for provider transcript homes, routed through
//! [`crate::observe::Cache`]'s single process-wide watcher. Events are hints:
//! they make the observation cache read a changed transcript's turn again, and
//! its reconcile interval catches what they miss. Every subscription reading
//! that transcript shares the one reading.
//!
//! OpenCode writes every conversation into one SQLite store, and FSEvents
//! reports no write to it while OpenCode holds it open. OpenCode snapshots
//! the worktree under the same home as each step starts and finishes, so any
//! event under its home hints each OpenCode conversation read; one read costs
//! a query on that store, a few milliseconds.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::observe::{Cache, TranscriptWatchGuard};

/// Watch each harness's shared home through `cache`'s single shared filesystem
/// watcher until the returned guard drops. A home that is missing or cannot be
/// watched is logged and left to the reconcile interval.
pub fn watch(cache: &Arc<Cache>) -> TranscriptWatchGuard {
    cache.watch_transcripts()
}

/// The transcripts an event on `paths` changes: `store` when the watch is
/// OpenCode's, else the `.jsonl` files among them. FSEvents may name a file
/// by its resolved path; facts keep the spelling they were read with, so both
/// are hinted.
pub(crate) fn transcripts(paths: &[PathBuf], store: Option<&Path>) -> Vec<PathBuf> {
    let changed = match store {
        Some(store) => vec![store.to_path_buf()],
        None => paths
            .iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .cloned()
            .collect(),
    };
    changed
        .into_iter()
        .flat_map(|path| {
            let resolved = path
                .canonicalize()
                .ok()
                .filter(|resolved| *resolved != path);
            std::iter::once(path).chain(resolved)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::time::SystemTime;

    use super::*;
    use loopflow_harness::transcript::live::TurnState;
    use loopflow_harness::HarnessKind;

    const RUNNING: &str = r#"{"type":"user","userType":"external","timestamp":"2026-11-10T16:25:37Z","message":{"content":"Explain the build"}}"#;
    const ENDED: &str = r#"{"type":"assistant","timestamp":"2026-11-10T16:27:51Z","message":{"id":"m","model":"claude-opus-5-5","stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":7}}}"#;

    #[test]
    fn a_grown_transcript_gives_every_subscriber_the_same_one_reading() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("projects/-work/c.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{RUNNING}\n")).unwrap();
        let cache = Arc::new(Cache::default());
        let source = loopflow_harness::transcript::Source::File(path.clone());
        let read = || crate::observe::transcript_turn(HarnessKind::Claude, &source);

        let subscribers = (0..8).map(|_| cache.capture(read)).collect::<Vec<_>>();
        assert!(subscribers
            .iter()
            .all(|(turn, _)| turn.as_ref().unwrap().state == TurnState::Running));

        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(format!("{ENDED}\n").as_bytes())
            .unwrap();
        cache.refresh_due_at(SystemTime::now());
        assert!(
            subscribers.iter().all(|(_, seen)| !seen.changed()),
            "the reconcile interval has not passed and no event arrived"
        );

        for hinted in transcripts(std::slice::from_ref(&path), None) {
            cache.hint_transcript(Some(&hinted), false);
        }
        // The hint makes it due after TURN_HINTED, not immediately.
        cache.refresh_due_at(SystemTime::now());
        assert!(subscribers.iter().all(|(_, seen)| !seen.changed()));

        cache.refresh_due_at(
            SystemTime::now() + crate::observe::TURN_HINTED + std::time::Duration::from_millis(10),
        );
        assert!(subscribers.iter().all(|(_, seen)| seen.changed()));
        let (after, _) = cache.capture(read);
        let after = after.unwrap();
        assert_eq!(after.state, TurnState::Ended);
        assert_eq!(after.usage.unwrap().output_tokens, 7);
    }

    #[test]
    fn only_transcript_files_are_hinted() {
        let found = transcripts(
            &[
                PathBuf::from("/nowhere/a.jsonl"),
                PathBuf::from("/nowhere/a"),
                PathBuf::from("/nowhere/settings.json"),
            ],
            None,
        );
        assert_eq!(found, [PathBuf::from("/nowhere/a.jsonl")]);
    }

    #[test]
    fn a_snapshot_under_opencodes_home_hints_its_store() {
        let store = Path::new("/nowhere/opencode/opencode.db");
        let snapshot = PathBuf::from("/nowhere/opencode/snapshot/e2c4/b0db/index");
        assert_eq!(transcripts(&[snapshot], Some(store)), [store.to_path_buf()]);
    }
}
