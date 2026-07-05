//! The push bridge: store-poll → EventHub (collapse call #4).
//!
//! Mutations now happen outside the daemon's process — `lf wave` registers
//! sessions store-direct, `lf q` writes runs, `lf op queue reconcile` writes
//! attention — and the in-process [`EventHub`] never sees them, so Concerto's
//! `/ws` feed starves for anything the daemon didn't do itself. The bridge
//! polls the store machine-wide (the [`crate::wave::registry::StoreObserver`]
//! shape, generalized), diffs waves/runs/sessions/attention against its last
//! snapshot, and emits the existing [`Event`] vocabulary for what changed.
//!
//! Constitutional: derived state only. The snapshot is in-memory, rebuilt at
//! boot — the first successful poll seeds it silently (existing rows are not
//! news), and a crash loses nothing that the next boot's seed doesn't
//! re-derive.
//!
//! Cost, honestly: none of these tables carry an `updated_at` column, so
//! every poll fully scans waves, sessions, and attention — acceptable for a
//! small machine-local db at a 5s cadence. Runs are the one table that grows
//! without bound (terminal history), so that scan is filtered: non-terminal
//! runs plus runs ended since the last poll (with one cadence of overlap —
//! a duplicate event converges, a missed one doesn't). Waves/sessions/
//! attention full scans are the next thing to revisit if their row counts
//! ever stop being small.
//!
//! Duplicates, honestly: the daemon still emits in-process events for its own
//! actions (until cut 4 removes the organs), so a daemon-side write is
//! announced twice — once live, once when the bridge sees the row. We accept
//! the duplicates rather than fingerprint against the hub: Concerto's
//! consumers are state-replacing (wave events apply the enriched DTO or
//! trigger a refetch; session/attention events are upserts or ignored), so a
//! repeated event converges to the same UI state.

// TODO(M1/M2): preserve these event-projection mechanics when the shared query
// plane owns them: silent boot seed, fingerprint diffs, bounded scans for
// growing tables, and duplicate-tolerant state replacement.
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::Duration;

use crate::lfd::events::EventHub;
use crate::lfd::types::{AttentionItem, AttentionStatus, Event};
use crate::lfdb::SharedStore;

/// How often the bridge re-reads the store.
pub const POLL_CADENCE: Duration = Duration::from_secs(5);

/// Everything the bridge remembers between polls. Row identity → a
/// fingerprint of the whole row (any field change flips it) plus whatever
/// the diff needs to name the event (wave name for creates, owning wave for
/// runs, resolution state for attention).
#[derive(Debug)]
struct Snapshot {
    /// When this snapshot's read started — the next poll's runs-scan floor.
    taken_at: time::OffsetDateTime,
    /// wave id → (fingerprint, name)
    waves: HashMap<String, (u64, String)>,
    /// run id → (fingerprint, wave id, terminal?). Holds only the filtered
    /// working set; a TERMINAL run aging out of the scan window is not news,
    /// a NON-terminal run vanishing is (deleted, or ended long ago with no
    /// `ended_at` to catch it).
    runs: HashMap<String, (u64, String, bool)>,
    /// session id → fingerprint
    sessions: HashMap<String, u64>,
    /// attention id → (fingerprint, resolved?)
    attention: HashMap<String, (u64, bool)>,
}

impl Snapshot {
    fn empty(taken_at: time::OffsetDateTime) -> Self {
        Self {
            taken_at,
            waves: HashMap::new(),
            runs: HashMap::new(),
            sessions: HashMap::new(),
            attention: HashMap::new(),
        }
    }
}

/// Polls the store and emits events for rows that changed since the last
/// poll. See the module doc for the seeding, cost, and duplicate story.
#[derive(Debug)]
pub struct StoreBridge {
    store: SharedStore,
    hub: EventHub,
    /// `None` until the first successful poll seeds it (silently).
    snapshot: Mutex<Option<Snapshot>>,
}

impl StoreBridge {
    pub fn new(store: SharedStore, hub: EventHub) -> Self {
        Self {
            store,
            hub,
            snapshot: Mutex::new(None),
        }
    }

    /// Poll forever on `cadence`. Runs until aborted at daemon shutdown.
    pub async fn run(self, cadence: Duration) {
        loop {
            self.poll_once().await;
            tokio::time::sleep(cadence).await;
        }
    }

    /// One diff pass. A failed read of any table skips the whole cycle
    /// without advancing the snapshot — a transient store error must never
    /// masquerade as a wave of deletions.
    pub async fn poll_once(&self) {
        let taken_at = time::OffsetDateTime::now_utc();
        // Runs-scan floor: everything non-terminal, plus runs ended since the
        // previous poll started (one extra cadence of overlap for reads that
        // raced the write — duplicates converge, misses don't). The seed poll
        // has no floor to honor; any value works, it only builds a baseline.
        let ended_since = {
            let guard = self.snapshot.lock().expect("bridge snapshot poisoned");
            guard.as_ref().map_or(taken_at, |prev| prev.taken_at) - POLL_CADENCE
        };
        let (waves, runs, sessions, attention) = match tokio::try_join!(
            async {
                self.store
                    .list_waves(None)
                    .await
                    .map_err(|err| err.to_string())
            },
            async {
                self.store
                    .list_runs_active_or_ended_since(ended_since)
                    .await
                    .map_err(|err| err.to_string())
            },
            async {
                self.store
                    .list_control_sessions(None, None)
                    .await
                    .map_err(|err| err.to_string())
            },
            async {
                self.store
                    .list_attention_items(None, None)
                    .await
                    .map_err(|err| err.to_string())
            },
        ) {
            Ok(rows) => rows,
            Err(err) => {
                tracing::debug!(error = %err, "push bridge store read failed; skipping cycle");
                return;
            }
        };

        let mut next = Snapshot::empty(taken_at);
        for wave in &waves {
            next.waves.insert(
                wave.id().to_string(),
                (fingerprint(wave), wave.name().clone()),
            );
        }
        for run in &runs {
            next.runs.insert(
                run.id.to_string(),
                (fingerprint(run), run.wave_id.to_string(), is_terminal(run)),
            );
        }
        for session in &sessions {
            next.sessions
                .insert(session.id.to_string(), fingerprint(session));
        }
        for item in &attention {
            next.attention
                .insert(item.id.to_string(), (fingerprint(item), is_resolved(item)));
        }

        let mut guard = self.snapshot.lock().expect("bridge snapshot poisoned");
        let Some(prev) = guard.as_ref() else {
            // Boot: existing rows are the baseline, not news.
            *guard = Some(next);
            return;
        };

        // Waves: created / updated / deleted by row identity + fingerprint.
        let mut waves_told: HashSet<String> = HashSet::new();
        for wave in &waves {
            let id = wave.id().to_string();
            match prev.waves.get(&id) {
                None => {
                    self.hub
                        .send(Event::wave_created(wave.id().clone(), wave.name().clone()));
                    waves_told.insert(id);
                }
                Some((fp, _)) if *fp != next.waves[&id].0 => {
                    self.hub.send(Event::wave_updated(wave.id().clone()));
                    waves_told.insert(id);
                }
                Some(_) => {}
            }
        }
        for (id, _) in prev.waves.iter() {
            if !next.waves.contains_key(id) {
                self.hub
                    .send(Event::wave_deleted(crate::lfd::id::LfdId::from_raw(id)));
            }
        }

        // Runs: any change (new row, status flip, PR attached, removal) is
        // announced as wave_updated for the owning wave — the /ws enrichment
        // attaches the full WaveDto, which is where run state lives for
        // consumers. Coalesced per wave per poll. A run leaving the filtered
        // set is news only if it was last seen NON-terminal (deleted, or
        // finished untracked); a terminal run aging out of the ended_since
        // window is expected and silent.
        let mut touched_waves: HashSet<String> = HashSet::new();
        for (run_id, (fp, wave_id, _)) in &next.runs {
            match prev.runs.get(run_id) {
                None => {
                    touched_waves.insert(wave_id.clone());
                }
                Some((prev_fp, _, _)) if prev_fp != fp => {
                    touched_waves.insert(wave_id.clone());
                }
                Some(_) => {}
            }
        }
        for (run_id, (_, wave_id, was_terminal)) in prev.runs.iter() {
            if !was_terminal && !next.runs.contains_key(run_id) {
                touched_waves.insert(wave_id.clone());
            }
        }
        for wave_id in touched_waves {
            if !waves_told.contains(&wave_id) && next.waves.contains_key(&wave_id) {
                self.hub
                    .send(Event::wave_updated(crate::lfd::id::LfdId::from_raw(
                        &wave_id,
                    )));
            }
        }

        // Sessions: created / updated (the vocabulary has no removal event).
        for session in &sessions {
            let id = session.id.to_string();
            match prev.sessions.get(&id) {
                None => self.hub.send(Event::session_created(session.clone())),
                Some(fp) if *fp != next.sessions[&id] => {
                    self.hub.send(Event::session_updated(session.clone()));
                }
                Some(_) => {}
            }
        }

        // Attention: created / updated / resolved. An item first seen already
        // resolved was born and settled within one poll gap — announce only
        // the resolution. Row deletion has no event kind; skipped.
        for item in &attention {
            let id = item.id.to_string();
            let resolved = is_resolved(item);
            match prev.attention.get(&id) {
                None if resolved => self.hub.send(Event::attention_resolved(item.clone())),
                None => self.hub.send(Event::attention_created(item.clone())),
                Some((fp, was_resolved)) if *fp != next.attention[&id].0 => {
                    if resolved && !was_resolved {
                        self.hub.send(Event::attention_resolved(item.clone()));
                    } else {
                        self.hub.send(Event::attention_updated(item.clone()));
                    }
                }
                Some(_) => {}
            }
        }

        *guard = Some(next);
    }
}

fn is_resolved(item: &AttentionItem) -> bool {
    item.status == AttentionStatus::Resolved
}

fn is_terminal(run: &crate::lfd::types::Run) -> bool {
    matches!(
        run.status,
        crate::lfd::types::RunStatus::Completed | crate::lfd::types::RunStatus::Failed
    )
}

/// A whole-row fingerprint via the row's `Debug` form — every persisted type
/// derives `Debug`, and any field change flips the hash. Stable only within
/// one process, which is all a boot-rebuilt snapshot needs.
fn fingerprint(row: &impl std::fmt::Debug) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{row:?}").hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        AttentionKind, RepoWork, Run, RunStackStatus, RunStatus, Session, SessionStatus,
        SessionUse, Wave, WaveMode, WaveStatus,
    };
    use crate::lfdb::{open_store, StorageConfig};

    async fn store_at(path: &std::path::Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(path.to_path_buf()))
                .await
                .expect("open sqlite store"),
        )
    }

    fn make_wave(name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repos: vec![RepoWork {
                repo: "/tmp/repo".to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_run(wave: &Wave) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: "/tmp/repo".to_string(),
            flow: "implement".to_string(),
            task: None,
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Running,
            worktree: "/tmp/repo.ship".to_string(),
            branch: "ship-branch".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }

    fn make_session(wave: &Wave) -> Session {
        let now = OffsetDateTime::now_utc();
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "dispatch:implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.ship".to_string(),
            argv: Vec::new(),
            env: std::collections::BTreeMap::new(),
            source: "tmux_terminal".to_string(),
            tmux_name: "lf-x".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(now),
            completed_at: None,
            created_at: now,
            completion_token: None,
        }
    }

    fn make_attention(wave: &Wave) -> AttentionItem {
        AttentionItem {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            kind: AttentionKind::Algedonic,
            status: AttentionStatus::Surfaced,
            title: "queue blocked".to_string(),
            summary: "scratch dirty".to_string(),
            context: serde_json::json!({}),
            surfaced_at: OffsetDateTime::now_utc(),
            viewed_at: None,
            resolved_at: None,
        }
    }

    fn event_types(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<String> {
        let mut types = Vec::new();
        while let Ok(event) = rx.try_recv() {
            let json = serde_json::to_value(&event).expect("event serializes");
            types.push(json["type"].as_str().expect("typed event").to_string());
        }
        types
    }

    /// Boot with existing rows: the first poll seeds the snapshot silently.
    #[tokio::test]
    async fn boot_with_existing_rows_emits_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = store_at(&tmp.path().join("lfd.db")).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("wave");
        store.create_run(&make_run(&wave)).await.expect("run");
        store
            .register_session(&make_session(&wave))
            .await
            .expect("session");
        store
            .upsert_attention_item(&make_attention(&wave))
            .await
            .expect("attention");

        let hub = EventHub::new(64);
        let mut rx = hub.subscribe();
        let bridge = StoreBridge::new(store, hub);
        bridge.poll_once().await;

        assert!(event_types(&mut rx).is_empty(), "seed poll is silent");
    }

    /// The starving-feed case: a second connection (another process in real
    /// life) mutates the store; the bridge announces it within one poll.
    #[tokio::test]
    async fn second_connection_writes_are_emitted_within_a_poll() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let daemon_store = store_at(&db).await;
        let other_process = store_at(&db).await;

        let hub = EventHub::new(64);
        let mut rx = hub.subscribe();
        let bridge = StoreBridge::new(daemon_store, hub);
        bridge.poll_once().await; // seed on empty

        // Everything below happens through the second connection.
        let wave = make_wave("ship");
        other_process.create_wave(&wave).await.expect("wave");
        other_process
            .register_session(&make_session(&wave))
            .await
            .expect("session");
        other_process
            .upsert_attention_item(&make_attention(&wave))
            .await
            .expect("attention");

        bridge.poll_once().await;
        let types = event_types(&mut rx);
        assert!(types.contains(&"wave_created".to_string()), "{types:?}");
        assert!(types.contains(&"session_created".to_string()), "{types:?}");
        assert!(
            types.contains(&"attention_created".to_string()),
            "{types:?}"
        );
    }

    /// Steady state must not storm: unchanged rows emit nothing, however
    /// often the poll repeats.
    #[tokio::test]
    async fn unchanged_state_emits_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = store_at(&tmp.path().join("lfd.db")).await;
        let wave = make_wave("ship");
        store.create_wave(&wave).await.expect("wave");
        store.create_run(&make_run(&wave)).await.expect("run");

        let hub = EventHub::new(64);
        let mut rx = hub.subscribe();
        let bridge = StoreBridge::new(store, hub);
        bridge.poll_once().await; // seed
        bridge.poll_once().await;
        bridge.poll_once().await;

        assert!(event_types(&mut rx).is_empty(), "no storm on quiet state");
    }

    /// The filtered runs scan: a completion is announced once (the ended_since
    /// window catches it), then the terminal run ages out of the working set
    /// SILENTLY — no phantom wave_updated when it leaves the window.
    #[tokio::test]
    async fn completed_run_is_announced_once_then_ages_out_silently() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let daemon_store = store_at(&db).await;
        let other_process = store_at(&db).await;

        let wave = make_wave("ship");
        let mut run = make_run(&wave);
        other_process.create_wave(&wave).await.expect("wave");
        other_process.create_run(&run).await.expect("run");

        let hub = EventHub::new(64);
        let mut rx = hub.subscribe();
        let bridge = StoreBridge::new(daemon_store, hub);
        bridge.poll_once().await; // seed

        run.status = RunStatus::Completed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        other_process.update_run(&run).await.expect("complete run");

        bridge.poll_once().await;
        let types = event_types(&mut rx);
        assert_eq!(
            types,
            vec!["wave_updated".to_string()],
            "completion announced once"
        );

        // The terminal run leaves the scan window across later polls: quiet.
        bridge.poll_once().await;
        bridge.poll_once().await;
        assert!(
            event_types(&mut rx).is_empty(),
            "terminal run ages out without phantom events"
        );
    }

    /// Run mutations surface as wave_updated (coalesced per wave); attention
    /// resolution surfaces as attention_resolved.
    #[tokio::test]
    async fn run_and_attention_transitions_emit_the_right_kinds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db = tmp.path().join("lfd.db");
        let daemon_store = store_at(&db).await;
        let other_process = store_at(&db).await;

        let wave = make_wave("ship");
        let mut run = make_run(&wave);
        let mut item = make_attention(&wave);
        other_process.create_wave(&wave).await.expect("wave");
        other_process.create_run(&run).await.expect("run");
        other_process
            .upsert_attention_item(&item)
            .await
            .expect("attention");

        let hub = EventHub::new(64);
        let mut rx = hub.subscribe();
        let bridge = StoreBridge::new(daemon_store, hub);
        bridge.poll_once().await; // seed with the rows present

        run.status = RunStatus::Completed;
        run.ended_at = Some(OffsetDateTime::now_utc());
        other_process.update_run(&run).await.expect("run update");
        item.status = AttentionStatus::Resolved;
        item.resolved_at = Some(OffsetDateTime::now_utc());
        other_process
            .upsert_attention_item(&item)
            .await
            .expect("attention update");

        bridge.poll_once().await;
        let types = event_types(&mut rx);
        assert_eq!(
            types.iter().filter(|t| *t == "wave_updated").count(),
            1,
            "one coalesced wave_updated: {types:?}"
        );
        assert!(
            types.contains(&"attention_resolved".to_string()),
            "{types:?}"
        );
        assert!(
            !types.contains(&"attention_updated".to_string()),
            "resolution is not a plain update: {types:?}"
        );
    }
}
