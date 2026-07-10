//! The mind's ear on the agent bus.
//!
//! The bus lives in the shared store, not in this process: `lf radio` INSERTs
//! a row and exits. A served mind is therefore just another subscriber. It
//! polls forward from a durable cursor, records the reports addressed to its
//! family in its own journal, and wakes its loop with them — the same fold as
//! before, reading a table instead of its own broadcast arm.
//!
//! Durability is the cursor, not the socket. A mind that was asleep when a
//! hand reported catches up on wake; restart it again and the cursor, not
//! luck, keeps the report at exactly one copy. A report published longer ago
//! than the sweep window is gone, and the cursor jump says so out loud in the
//! thread — the PR and the run ledger stay the records of record.
//!
//! Self-speech is not a report: rows whose byline is the wave's own channel
//! (a mind steering one of its hands, or speaking in its own home) are read
//! and skipped, so a mind never wakes itself.

use std::sync::Arc;
use std::time::Duration;

use crate::lfdb::{BusMessage, SharedStore};
use crate::wave::journal::Attribution;
use crate::wave::runtime::WaveRuntime;

/// How often a subscriber sweeps the bus forward. Fast enough that a steer
/// feels live; slow enough that an idle machine is asleep.
pub const POLL_CADENCE: Duration = Duration::from_millis(250);

/// A served mind's subscription to the bus.
#[derive(Debug)]
pub struct BusListener {
    runtime: Arc<WaveRuntime>,
    store: SharedStore,
    /// Durable cursor key. The wave's channel name — one mind, one ear.
    subscriber: String,
    cursor: tokio::sync::Mutex<i64>,
}

impl BusListener {
    pub fn new(runtime: Arc<WaveRuntime>, store: SharedStore) -> Self {
        let subscriber = runtime.channel_name().to_string();
        Self {
            runtime,
            store,
            subscriber,
            cursor: tokio::sync::Mutex::new(0),
        }
    }

    /// Load the durable cursor (first boot tunes in at the head — a fresh mind
    /// has no backlog to answer) and report a swept-away gap into the thread.
    pub async fn attach(&self) -> anyhow::Result<()> {
        let stored = self.store.bus_cursor(self.subscriber.clone()).await?;
        let cursor = match stored {
            Some(cursor) => {
                self.report_gap(cursor).await?;
                cursor
            }
            None => self.store.bus_head().await?,
        };
        *self.cursor.lock().await = cursor;
        self.store
            .set_bus_cursor(self.subscriber.clone(), cursor)
            .await?;
        Ok(())
    }

    /// Poll forever on `cadence`. Runs until aborted at server shutdown.
    pub async fn run(self: Arc<Self>, cadence: Duration) {
        if let Err(err) = self.attach().await {
            tracing::warn!(error = %err, "bus subscription failed to attach; the mind is deaf");
            return;
        }
        loop {
            if let Err(err) = self.poll_once().await {
                tracing::warn!(error = %err, "bus poll failed; retrying");
            }
            tokio::time::sleep(cadence).await;
        }
    }

    /// One pass: fold every fresh in-family report into the journal, then
    /// commit the cursor. Delivery precedes the commit, so a crash mid-fold
    /// re-reads a row rather than losing it.
    pub async fn poll_once(&self) -> anyhow::Result<()> {
        let mut cursor = self.cursor.lock().await;
        let messages = self.store.read_bus_after(*cursor).await?;
        for message in &messages {
            if self.hears(message) {
                self.runtime.deliver_say(
                    message.text.clone(),
                    Attribution {
                        session_id: None,
                        label: message.byline.clone(),
                    },
                );
            }
            *cursor = message.id;
            self.store
                .set_bus_cursor(self.subscriber.clone(), *cursor)
                .await?;
        }
        Ok(())
    }

    /// A report this mind should record: spoken on its family's channels by
    /// someone other than itself.
    fn hears(&self, message: &BusMessage) -> bool {
        self.runtime.in_family(&message.channel) && message.byline != self.subscriber
    }

    /// A durable cursor below the oldest surviving frame means the sweeper
    /// reached reports this mind never read. Say so in the thread — a silent
    /// miss is the failure mode of treating a wire like a log.
    async fn report_gap(&self, cursor: i64) -> anyhow::Result<()> {
        let Some(floor) = self.store.bus_floor().await? else {
            return Ok(());
        };
        if floor <= cursor + 1 {
            return Ok(());
        }
        let missed = floor - cursor - 1;
        self.runtime.deliver_say(
            format!(
                "bus cursor jumped {cursor} → {floor}: {missed} broadcast(s) aged past the \
                 sweep window while this mind was asleep. The PRs and `lf runs` hold what \
                 the bus dropped."
            ),
            Attribution {
                session_id: None,
                label: "bus".to_string(),
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfdb::{open_store, StorageConfig};

    async fn temp_store(dir: &std::path::Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(dir.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    /// The whole sleeping-mind path: a hand publishes with no server running,
    /// the mind boots, and the report lands in its thread exactly once —
    /// across two restarts, because the cursor decides.
    #[tokio::test]
    async fn a_sleeping_mind_catches_up_exactly_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();

        // First boot tunes in at the head: nothing to answer.
        let runtime = WaveRuntime::open("ship".into(), origin.clone()).expect("runtime");
        let listener = BusListener::new(runtime.clone(), store.clone());
        listener.attach().await.expect("attach");
        listener.poll_once().await.expect("poll");
        assert!(runtime.thread_snapshot().is_empty());
        drop(runtime);

        // The mind is down; a hand reports anyway.
        store
            .publish_bus("ship.148e".into(), "ship.148e".into(), "landed PR".into())
            .await
            .expect("publish");

        // It wakes and catches up.
        let runtime = WaveRuntime::open("ship".into(), origin.clone()).expect("runtime");
        let listener = BusListener::new(runtime.clone(), store.clone());
        listener.attach().await.expect("attach");
        listener.poll_once().await.expect("poll");
        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "landed PR");
        assert_eq!(thread[0].from.as_deref(), Some("ship.148e"));

        // Restart again: the cursor, not luck, keeps it at one copy.
        drop(runtime);
        let runtime = WaveRuntime::open("ship".into(), origin).expect("runtime");
        let listener = BusListener::new(runtime.clone(), store.clone());
        listener.attach().await.expect("attach");
        listener.poll_once().await.expect("poll");
        assert_eq!(runtime.thread_snapshot().len(), 1, "exactly once");
    }

    /// Out-of-family channels are somebody else's traffic; a mind's own
    /// byline is its own voice. Neither reaches the thread.
    #[tokio::test]
    async fn the_mind_folds_family_reports_and_ignores_its_own_voice() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let runtime = WaveRuntime::open("ship".into(), origin).expect("runtime");
        let listener = BusListener::new(runtime.clone(), store.clone());
        listener.attach().await.expect("attach");

        for (channel, byline, text) in [
            ("goals", "goals", "another family"),
            ("ship.148e", "ship", "the mind steering its hand"),
            ("ship.148e", "ship.148e", "the hand reporting"),
            ("ship", "concerto", "a child wave escalating"),
        ] {
            store
                .publish_bus(channel.into(), byline.into(), text.into())
                .await
                .expect("publish");
        }
        listener.poll_once().await.expect("poll");

        let thread = runtime.thread_snapshot();
        let texts: Vec<&str> = thread.iter().map(|turn| turn.text.as_str()).collect();
        assert_eq!(texts, vec!["the hand reporting", "a child wave escalating"]);
    }

    /// A report swept away before the mind woke is missed, and the miss is
    /// visible in the thread rather than silent.
    #[tokio::test]
    async fn a_swept_report_leaves_a_visible_cursor_jump() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(&origin).unwrap();
        let runtime = WaveRuntime::open("ship".into(), origin).expect("runtime");

        // The mind slept with its cursor at the head of an empty bus. Two
        // reports arrived and aged past the window; a third survives.
        store
            .set_bus_cursor("ship".into(), 0)
            .await
            .expect("cursor");
        for text in ["one", "two"] {
            store
                .publish_bus("ship.a".into(), "ship.a".into(), text.into())
                .await
                .expect("publish");
        }
        let cutoff = time::OffsetDateTime::now_utc().unix_timestamp() + 1;
        assert_eq!(store.sweep_bus(cutoff).await.expect("sweep"), 2);
        store
            .publish_bus("ship.a".into(), "ship.a".into(), "three".into())
            .await
            .expect("publish");

        let listener = Arc::new(BusListener::new(runtime.clone(), store.clone()));
        listener.attach().await.expect("attach");
        listener.poll_once().await.expect("poll");

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 2, "the jump note, then the surviving report");
        assert_eq!(thread[0].from.as_deref(), Some("bus"));
        assert!(
            thread[0].text.contains("cursor jumped 0 → 3")
                && thread[0].text.contains("2 broadcast(s)"),
            "the miss names what it lost: {}",
            thread[0].text
        );
        assert_eq!(thread[1].text, "three");
    }
}
