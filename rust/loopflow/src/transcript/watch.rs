//! One filesystem watch per provider home, held by the Machine server. Events
//! are hints: they make the shared observation cache read a changed
//! transcript's turn again, and its reconcile interval catches what they miss.
//! Every subscription reading that transcript shares the one reading.
//!
//! OpenCode writes every conversation into one SQLite store, so any write to
//! it hints each OpenCode conversation read; one read costs a query on that
//! store, a few milliseconds.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use notify::{RecursiveMode, Watcher};

use crate::harness::HarnessKind;
use crate::observe::Cache;

/// Watch each harness's shared home until the returned watchers drop. A home
/// that is missing or cannot be watched is logged and left to the reconcile
/// interval.
pub(crate) fn watch(cache: &Arc<Cache>) -> Vec<notify::RecommendedWatcher> {
    [
        super::home(HarnessKind::Claude).join("projects"),
        super::home(HarnessKind::Codex).join("sessions"),
        super::home(HarnessKind::Pi).join("sessions"),
        super::home(HarnessKind::OpenCode),
    ]
    .iter()
    .filter(|root| root.is_dir())
    .filter_map(|root| match watch_root(root, Arc::downgrade(cache)) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            tracing::warn!(root = %root.display(), %error, "Could not watch transcripts");
            None
        }
    })
    .collect()
}

fn watch_root(root: &Path, cache: Weak<Cache>) -> notify::Result<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Some(cache) = cache.upgrade() {
            hint(&cache, event);
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

fn hint(cache: &Cache, event: notify::Result<notify::Event>) {
    let event = match event {
        Ok(event) if event.kind.is_access() => return,
        Ok(event) if !event.need_rescan() => event,
        Ok(_) | Err(_) => return cache.hint_transcript(None, true),
    };
    let created = event.kind.is_create();
    for path in transcripts(&event.paths) {
        // A new OpenCode conversation is a row, not a file; raw discovery
        // finds it at its own interval.
        let created = created && path.extension().is_some_and(|ext| ext == "jsonl");
        cache.hint_transcript(Some(&path), created);
    }
}

/// The transcripts `paths` change: a `.jsonl` file, or OpenCode's store for
/// a write to it or its log. FSEvents may name a file by its resolved path;
/// facts keep the spelling they were read with, so both are hinted.
fn transcripts(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            if name.ends_with(".jsonl") {
                return Some(path.clone());
            }
            let store = name.strip_suffix("-wal").unwrap_or(name);
            (store == "opencode.db").then(|| path.with_file_name(store))
        })
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

    use super::*;
    use crate::harness::HarnessKind;
    use crate::transcript::live::TurnState;

    const RUNNING: &str = r#"{"type":"user","userType":"external","timestamp":"2026-11-10T16:25:37Z","message":{"content":"Explain the build"}}"#;
    const ENDED: &str = r#"{"type":"assistant","timestamp":"2026-11-10T16:27:51Z","message":{"id":"m","model":"claude-opus-5-5","stop_reason":"end_turn","usage":{"input_tokens":5,"output_tokens":7}}}"#;

    #[test]
    fn a_grown_transcript_gives_every_subscriber_the_same_one_reading() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("projects/-work/c.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{RUNNING}\n")).unwrap();
        let cache = Arc::new(Cache::default());
        let source = crate::transcript::Source::File(path.clone());
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
        cache.refresh_local();
        assert!(
            subscribers.iter().all(|(_, seen)| !seen.changed()),
            "the reconcile interval has not passed and no event arrived"
        );

        hint(
            &cache,
            Ok(
                notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Any))
                    .add_path(path.clone()),
            ),
        );
        // The hint makes it due a second from now, not at once.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        cache.refresh_local();
        assert!(subscribers.iter().all(|(_, seen)| seen.changed()));
        let (after, _) = cache.capture(read);
        let after = after.unwrap();
        assert_eq!(after.state, TurnState::Ended);
        assert_eq!(after.usage.unwrap().output_tokens, 7);
    }

    #[test]
    fn only_transcript_files_are_hinted() {
        let found = transcripts(&[
            PathBuf::from("/nowhere/a.jsonl"),
            PathBuf::from("/nowhere/a"),
            PathBuf::from("/nowhere/settings.json"),
            PathBuf::from("/nowhere/opencode.db-wal"),
            PathBuf::from("/nowhere/opencode.db-shm"),
        ]);
        assert_eq!(
            found,
            [
                PathBuf::from("/nowhere/a.jsonl"),
                PathBuf::from("/nowhere/opencode.db")
            ]
        );
    }
}
