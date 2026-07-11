//! The keeper's watch over its tenant: resident-process liveness, the
//! process-level respawn ladder, and the interrupt-deadline janitor.
//!
//! The listener is the pen-holder, so every anti-wedge that must not depend
//! on the resident's health lives here, server-side:
//!
//! - **Resident death.** A spawned resident is watched by process exit; a
//!   separately attached one by a pid probe on the seat
//!   the attach door recorded. Death closes whatever turn the resident left
//!   open (`TurnFinished{Failed}` — the journal never dangles), journals
//!   `LoopState::Failed`, and arms the respawn ladder.
//! - **Respawn ladder.** Process-level auto-revival: attempt N waits the Nth
//!   rung (5m/15m/45m by default, the last rung repeating; an empty ladder
//!   disables it). A completed assistant turn resets the ladder. A human
//!   message revives a dead resident immediately, ladder or no ladder —
//!   talking to the wave brings it back. An ATTACHED resident is a revival
//!   too: the attach door signals the supervisor ([`SupervisorHandle`]),
//!   which disarms and resets the ladder and probes the seat by pid; and a
//!   spawn never fires over a live seat (probe before spawn).
//! - **Interrupt janitor.** When an interrupt op is delivered while a turn is
//!   live, a deadline arms. The RESIDENT kills the pass child and closes the
//!   turn through the wire immediately; this janitor is the backstop for a
//!   resident gone fully silent: past the deadline the open turn is
//!   force-finalized `Interrupted` and late wire deltas for it are dropped
//!   (see `WaveRuntime::force_finalize_open_turn`).
//!
//! The supervisor never touches a vendor: it spawns and kills `lf` processes
//! and folds what it observes into the journal.

use std::sync::Arc;
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;

use crate::chat::turns::{ChatRole, ChatTurn};
use crate::chat::types::Lifecycle;
use crate::wave::journal::MessageOp;
use crate::wave::registry::process_alive;
use crate::wave::runtime::{InboxItem, TurnFrame, WaveRuntime};
use crate::wave::server::ResidentDoor;
use crate::wave::state::LoopState;

/// Default respawn ladder: attempt N (0-based) waits the Nth rung; past the
/// end, the last rung repeats. Reset by a completed assistant turn.
pub const RESPAWN_BACKOFF: [Duration; 3] = [
    Duration::from_secs(5 * 60),
    Duration::from_secs(15 * 60),
    Duration::from_secs(45 * 60),
];

/// The listener-side interrupt janitor bound. The resident kills the pass
/// child and closes the turn through the wire immediately; this fires only
/// when the resident is silent.
pub const LISTENER_INTERRUPT_DEADLINE: Duration = Duration::from_secs(20);

/// How often an attached (not spawned) resident's pid is probed.
pub const ATTACH_PROBE: Duration = Duration::from_secs(10);

/// Spawn one resident process. Production spawns `lf __resident <name>` with the
/// resident endpoint/token in its environment
/// (the current executable); tests spawn whatever stands in for a resident.
/// A closure, not a trait: the supervisor needs exactly one behavior.
pub type SpawnResident = Box<dyn FnMut() -> std::io::Result<Child> + Send>;

/// Supervisor knobs. `Default` is production.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// Respawn ladder; empty disables auto-respawn (a human message is then
    /// the only revival).
    pub respawn_backoff: Vec<Duration>,
    /// Interrupt janitor bound (see [`LISTENER_INTERRUPT_DEADLINE`]).
    pub interrupt_deadline: Duration,
    /// Pid-probe cadence for attached residents (see [`ATTACH_PROBE`]).
    pub attach_probe: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            respawn_backoff: RESPAWN_BACKOFF.to_vec(),
            interrupt_deadline: LISTENER_INTERRUPT_DEADLINE,
            attach_probe: ATTACH_PROBE,
        }
    }
}

/// The delay before respawn attempt number `attempts_made + 1`. `None` iff
/// the ladder is empty — auto-respawn disabled.
fn respawn_delay(backoff: &[Duration], attempts_made: u32) -> Option<Duration> {
    backoff
        .get(attempts_made as usize)
        .or(backoff.last())
        .copied()
}

async fn wait_child(child: &mut Option<Child>) -> std::io::Result<std::process::ExitStatus> {
    child
        .as_mut()
        .expect("guarded by if in select")
        .wait()
        .await
}

/// Sleep until an optional deadline; `None` never fires. Shared with the
/// loop's scheduler loop ([`crate::flowloop::wave`]).
pub(crate) async fn sleep_until_opt(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// The listener's line into the keeper: the attach door signals the
/// supervisor so an attached resident stands the respawn ladder down.
/// Obtained from [`Supervisor::handle`] before the supervisor is spawned;
/// cloneable into the router state.
#[derive(Debug, Clone)]
pub struct SupervisorHandle {
    attach_tx: mpsc::UnboundedSender<u32>,
}

impl SupervisorHandle {
    /// A resident attached through the door (its pid already holds the
    /// seat). Fire-and-forget: the supervisor may already be gone at
    /// shutdown.
    pub fn on_attach(&self, pid: u32) {
        let _ = self.attach_tx.send(pid);
    }
}

/// The keeper's loop state. Construct with [`Supervisor::new`] — it
/// subscribes to the runtime's broadcasts synchronously, so ops delivered
/// after construction are never missed even before the task is first polled —
/// then drive with [`Supervisor::run`] (typically `tokio::spawn(sup.run())`).
pub struct Supervisor {
    runtime: Arc<WaveRuntime>,
    door: ResidentDoor,
    spawner: Option<SpawnResident>,
    config: SupervisorConfig,
    inbox_rx: broadcast::Receiver<InboxItem>,
    state_rx: broadcast::Receiver<LoopState>,
    turn_rx: broadcast::Receiver<Arc<TurnFrame>>,
    /// Attach signals from the listener's door (see [`SupervisorHandle`]).
    /// The paired sender is kept alive by `handle`, so `recv` never closes.
    attach_rx: mpsc::UnboundedReceiver<u32>,
    attach_tx: mpsc::UnboundedSender<u32>,
    /// The spawned resident, when this supervisor owns spawning and one is
    /// believed alive. `None` for attached residents (probed by pid).
    child: Option<Child>,
    /// When the next auto-respawn fires; set on resident death, cleared by
    /// any spawn.
    respawn_at: Option<Instant>,
    /// Respawn attempts since the last completed turn — the ladder index.
    attempts: u32,
    /// The interrupt janitor's deadline; armed when an interrupt op arrives
    /// with a turn live, cleared when the loop settles idle.
    interrupt_at: Option<Instant>,
}

impl Supervisor {
    /// Build the supervisor, subscribing to the runtime's broadcasts now.
    /// `spawner` is `None` only in listener-only tests: the janitor and
    /// the attach probe still run — the pen-side anti-wedges never depend on
    /// who spawned the resident.
    pub fn new(
        runtime: Arc<WaveRuntime>,
        door: ResidentDoor,
        spawner: Option<SpawnResident>,
        config: SupervisorConfig,
    ) -> Self {
        let (attach_tx, attach_rx) = mpsc::unbounded_channel();
        Self {
            inbox_rx: runtime.subscribe_inbox(),
            state_rx: runtime.subscribe_states(),
            turn_rx: runtime.subscribe_turns(),
            runtime,
            door,
            spawner,
            config,
            attach_rx,
            attach_tx,
            child: None,
            respawn_at: None,
            attempts: 0,
            interrupt_at: None,
        }
    }

    /// A handle for the attach door, taken before `run` consumes self.
    pub fn handle(&self) -> SupervisorHandle {
        SupervisorHandle {
            attach_tx: self.attach_tx.clone(),
        }
    }

    /// Run until aborted (server shutdown).
    pub async fn run(mut self) {
        if self.spawner.is_some() {
            self.spawn().await;
        }
        let mut probe = tokio::time::interval(self.config.attach_probe);
        probe.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                status = wait_child(&mut self.child), if self.child.is_some() => {
                    let reason = match status {
                        Ok(status) => format!("resident process exited ({status})"),
                        Err(err) => format!("resident process unwaitable: {err}"),
                    };
                    self.child = None;
                    self.on_resident_death(&reason);
                }
                item = self.inbox_rx.recv() => {
                    match item {
                        Ok(item) => self.on_inbox(item).await,
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                attached = self.attach_rx.recv() => {
                    if let Some(pid) = attached {
                        self.on_attach(pid);
                    }
                }
                state = self.state_rx.recv() => {
                    match state {
                        Ok(LoopState::Idle) => self.interrupt_at = None,
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // Resync from the fold: an Idle we missed still
                            // stands down the janitor.
                            if self.runtime.loop_state() == LoopState::Idle {
                                self.interrupt_at = None;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                turn = self.turn_rx.recv() => {
                    if let Ok(turn) = turn {
                        self.on_turn_frame(&turn.turn);
                    }
                }
                _ = sleep_until_opt(self.interrupt_at), if self.interrupt_at.is_some() => {
                    self.interrupt_at = None;
                    tracing::error!(
                        wave = self.runtime.name(),
                        "interrupt deadline expired with the resident silent; force-finalizing"
                    );
                    self.runtime.force_finalize_open_turn(
                        Lifecycle::Interrupted,
                        "interrupt deadline: resident silent; listener force-finalized",
                    );
                }
                _ = sleep_until_opt(self.respawn_at), if self.respawn_at.is_some() => {
                    self.respawn_at = None;
                    self.attempts += 1;
                    tracing::info!(
                        wave = self.runtime.name(),
                        attempt = self.attempts,
                        "auto-respawning the resident"
                    );
                    self.spawn().await;
                }
                _ = probe.tick() => {
                    self.probe_attached().await;
                }
            }
        }
    }

    async fn spawn(&mut self) {
        if self.spawner.is_none() {
            return;
        }
        self.respawn_at = None;
        // Never spawn over a live seat: a resident may have attached while
        // the ladder was armed (the attach signal disarms it, but a deadline
        // already due can race the signal). The attached resident IS the
        // revival; spawning would seat a second loop and orphan the first.
        if self.child.is_none() {
            if let Some(pid) = self.door.seat_pid() {
                if process_alive(pid).await {
                    tracing::info!(
                        wave = self.runtime.name(),
                        pid,
                        "a live resident already holds the seat; not spawning"
                    );
                    return;
                }
            }
        }
        let spawner = self.spawner.as_mut().expect("checked above");
        match spawner() {
            Ok(child) => {
                if let Some(pid) = child.id() {
                    self.door.record_pid(pid);
                }
                self.runtime.set_resident_expected();
                self.child = Some(child);
            }
            Err(err) => {
                let reason = format!("resident spawn failed: {err}");
                tracing::error!(wave = self.runtime.name(), error = %err, "resident spawn failed");
                self.mark_failed(&reason);
                if let Some(delay) = respawn_delay(&self.config.respawn_backoff, self.attempts) {
                    self.respawn_at = Some(Instant::now() + delay);
                }
            }
        }
    }

    /// A resident (spawned or attached) is gone: free the seat, close
    /// whatever it left open, journal the failure, arm the ladder.
    fn on_resident_death(&mut self, reason: &str) {
        tracing::error!(wave = self.runtime.name(), reason, "resident died");
        self.door.clear_seat();
        self.interrupt_at = None;
        self.runtime
            .force_finalize_open_turn(Lifecycle::Failed, "resident died mid-turn");
        self.mark_failed(reason);
        if self.spawner.is_some() {
            if let Some(delay) = respawn_delay(&self.config.respawn_backoff, self.attempts) {
                self.respawn_at = Some(Instant::now() + delay);
            }
        }
    }

    /// Journal `LoopState::Failed` unless the resident already reported it
    /// (the normal failure path reports Failed over the wire, then exits).
    fn mark_failed(&self, reason: &str) {
        if !matches!(self.runtime.loop_state(), LoopState::Failed { .. }) {
            self.runtime.transition(
                LoopState::Failed {
                    reason: reason.to_string(),
                },
                reason,
            );
        }
    }

    async fn on_inbox(&mut self, item: InboxItem) {
        let is_interrupt = match &item {
            InboxItem::Interrupt | InboxItem::Skip => true,
            InboxItem::Message(message) => {
                // A human message revives a dead resident immediately —
                // ladder or no ladder ("interrupt & send" included).
                if self.spawner.is_some()
                    && self.child.is_none()
                    && matches!(self.runtime.loop_state(), LoopState::Failed { .. })
                {
                    tracing::info!(
                        wave = self.runtime.name(),
                        "message for a dead resident; respawning now"
                    );
                    self.spawn().await;
                }
                message.op == MessageOp::Interrupt
            }
            InboxItem::Task(_) => {
                if self.spawner.is_some()
                    && self.child.is_none()
                    && matches!(self.runtime.loop_state(), LoopState::Failed { .. })
                {
                    tracing::info!(
                        wave = self.runtime.name(),
                        "Task observation for a dead resident; respawning now"
                    );
                    self.spawn().await;
                }
                false
            }
        };
        // The janitor arms only while a turn is live; an interrupt while idle
        // is a no-op (nothing to force).
        if is_interrupt
            && self.interrupt_at.is_none()
            && matches!(
                self.runtime.loop_state(),
                LoopState::Turning { .. } | LoopState::Interrupting { .. }
            )
        {
            self.interrupt_at = Some(Instant::now() + self.config.interrupt_deadline);
        }
    }

    /// The attach door admitted a resident (its pid already on the seat).
    /// For a separately attached resident, not our own spawned child, the
    /// attach is the revival: the respawn ladder stands
    /// down and resets, and the seat is watched by pid probe
    /// (attached-not-child). Without this, an armed respawn deadline would
    /// later spawn a second loop over the attached one and overwrite its
    /// seat pid.
    fn on_attach(&mut self, pid: u32) {
        if self
            .child
            .as_ref()
            .is_some_and(|child| child.id() == Some(pid))
        {
            // Our own spawned child announcing itself; the exit-watch
            // already covers it and the ladder state is spawn's business.
            return;
        }
        if let Some(mut child) = self.child.take() {
            // Defensive: the door seated a foreign resident over a live
            // spawned child. One seat, one loop — ask the replaced child to
            // leave, and reap it off-loop so its exit is never journaled as
            // a death.
            if let Some(old_pid) = child.id() {
                tracing::warn!(
                    wave = self.runtime.name(),
                    old_pid,
                    new_pid = pid,
                    "attach replaced a spawned resident; terminating the old one"
                );
                tokio::spawn(async move {
                    terminate_resident(old_pid).await;
                    let _ = child.wait().await;
                });
            }
        }
        self.respawn_at = None;
        self.attempts = 0;
        tracing::info!(
            wave = self.runtime.name(),
            pid,
            "resident attached; respawn stood down"
        );
    }

    fn on_turn_frame(&mut self, turn: &ChatTurn) {
        if turn.role == ChatRole::Assistant && turn.status == Lifecycle::Completed {
            // A completed turn resets the respawn ladder.
            self.attempts = 0;
        }
    }

    /// Attached residents have no child handle: probe the seat's pid.
    async fn probe_attached(&mut self) {
        if self.child.is_some() {
            return;
        }
        let Some(pid) = self.door.seat_pid() else {
            return;
        };
        if !process_alive(pid).await {
            self.on_resident_death("resident pid gone");
        }
    }
}

/// Ask a resident to leave: SIGTERM (its interrupt hooks stop the harness and
/// kill the vendor process group), a short grace, then SIGKILL. Used by the
/// listener's shutdown path.
pub async fn terminate_resident(pid: u32) {
    signal_pid(pid, "-TERM");
    for _ in 0..30 {
        if !process_alive(pid).await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tracing::warn!(pid, "resident ignored SIGTERM; killing");
    signal_pid(pid, "-KILL");
}

/// Best-effort SIGTERM for synchronous contexts (the Ctrl-C interrupt hook
/// runs on a plain thread).
pub fn terminate_resident_blocking(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
}

fn signal_pid(pid: u32, signal: &str) {
    let _ = std::process::Command::new("kill")
        .args([signal, &pid.to_string()])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    use crate::wave::journal::{journal_path, EventKind, Journal};
    use crate::wave::wire::ResidentDelta;

    fn open_runtime(repo: &std::path::Path) -> Arc<WaveRuntime> {
        WaveRuntime::open("ship".into(), repo.to_path_buf()).expect("open runtime")
    }

    /// A spawner running `sh -c <script>` with a counter — the mock resident
    /// is a real process, so exit detection is the production path.
    fn counting_spawner(script: &'static str) -> (SpawnResident, Arc<AtomicU32>) {
        let count = Arc::new(AtomicU32::new(0));
        let counter = count.clone();
        let spawner: SpawnResident = Box::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::process::Command::new("sh")
                .args(["-c", script])
                .kill_on_drop(true)
                .spawn()
        });
        (spawner, count)
    }

    fn config(backoff: Vec<Duration>) -> SupervisorConfig {
        SupervisorConfig {
            respawn_backoff: backoff,
            interrupt_deadline: Duration::from_millis(80),
            attach_probe: Duration::from_millis(40),
        }
    }

    async fn wait_for(what: &str, cond: impl Fn() -> bool) {
        for _ in 0..500 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition not met in time: {what}");
    }

    #[test]
    fn respawn_delay_walks_the_ladder_and_caps() {
        let ladder = RESPAWN_BACKOFF.to_vec();
        assert_eq!(respawn_delay(&ladder, 0), Some(Duration::from_secs(300)));
        assert_eq!(respawn_delay(&ladder, 1), Some(Duration::from_secs(900)));
        assert_eq!(respawn_delay(&ladder, 2), Some(Duration::from_secs(2700)));
        assert_eq!(
            respawn_delay(&ladder, 9),
            Some(Duration::from_secs(2700)),
            "past the ladder the cap repeats"
        );
        assert_eq!(respawn_delay(&[], 0), None, "empty ladder disables");
    }

    /// A dying resident process is detected, journaled as a failed loop, and
    /// respawned on the ladder — process-level auto-revival.
    #[tokio::test]
    async fn resident_death_fails_the_loop_and_the_ladder_respawns() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let (spawner, spawns) = counting_spawner("exit 0");
        let task = tokio::spawn(
            Supervisor::new(
                rt.clone(),
                door,
                Some(spawner),
                config(vec![Duration::from_millis(30)]),
            )
            .run(),
        );

        // First spawn dies instantly → Failed; the ladder respawns at least
        // twice more.
        wait_for("loop failed", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        assert!(rt.resident_expected(), "spawn marks the resident expected");
        wait_for("respawns happened", || spawns.load(Ordering::SeqCst) >= 3).await;
        task.abort();

        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("journal");
        assert!(
            events.iter().any(|e| matches!(
                &e.kind,
                EventKind::LoopState { to: LoopState::Failed { reason }, .. }
                    if reason.contains("resident process exited")
            )),
            "the death is journaled with its reason"
        );
    }

    /// A resident dying mid-turn never leaves the journal dangling: the open
    /// turn force-finalizes `Failed` before the loop is marked failed.
    #[tokio::test]
    async fn resident_death_closes_the_open_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let (spawner, _spawns) = counting_spawner("sleep 0.3");
        let task = tokio::spawn(
            Supervisor::new(rt.clone(), door, Some(spawner), config(Vec::new())).run(),
        );

        // The resident opens a turn, then its process dies.
        rt.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        rt.apply_resident_delta(ResidentDelta::TurnText {
            text: "half".into(),
        });
        wait_for("loop failed", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        task.abort();

        let thread = rt.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].status, Lifecycle::Failed, "open turn closed");
        assert_eq!(thread[0].text, "half");

        // And a late wire delta for that turn is dropped, not journaled.
        rt.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        assert_eq!(rt.thread_snapshot().len(), 1);
        assert!(matches!(rt.loop_state(), LoopState::Failed { .. }));
    }

    /// A human message revives a dead resident immediately, even when the
    /// ladder's next rung is far away.
    #[tokio::test]
    async fn human_message_respawns_a_dead_resident_immediately() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let (spawner, spawns) = counting_spawner("exit 0");
        let task = tokio::spawn(
            Supervisor::new(
                rt.clone(),
                door,
                Some(spawner),
                config(vec![Duration::from_secs(3600)]),
            )
            .run(),
        );

        wait_for("loop failed", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        let before = spawns.load(Ordering::SeqCst);
        rt.deliver(MessageOp::Message, "are you alive?".into())
            .expect("user turn");
        wait_for("immediate respawn", || {
            spawns.load(Ordering::SeqCst) > before
        })
        .await;
        task.abort();
    }

    /// The listener-side janitor: an interrupt delivered while a turn is
    /// live, with the resident fully silent, force-finalizes at the deadline.
    #[tokio::test]
    async fn interrupt_deadline_force_finalizes_when_the_resident_is_silent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let task = tokio::spawn(Supervisor::new(rt.clone(), door, None, config(Vec::new())).run());

        rt.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        rt.apply_resident_delta(ResidentDelta::TurnText {
            text: "half".into(),
        });
        // Give the supervisor a beat to subscribe... it subscribed before the
        // loop; deliver the interrupt and wait for the force-path.
        rt.deliver_interrupt();
        wait_for("force-finalized interrupted", || {
            rt.thread_snapshot()
                .last()
                .is_some_and(|t| t.status == Lifecycle::Interrupted)
        })
        .await;
        assert_eq!(rt.loop_state(), LoopState::Idle);
        task.abort();

        // The journal is closed; a replay agrees.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("journal");
        let fold = crate::wave::journal::fold_thread(&events);
        assert!(fold.open.is_empty());
        assert_eq!(fold.turns.last().unwrap().status, Lifecycle::Interrupted);
    }

    /// An attach through the door disarms an armed respawn: the attached
    /// resident IS the revival, and the deadline must not spawn a second
    /// loop over it (nor overwrite its seat pid).
    #[tokio::test]
    async fn attach_disarms_the_respawn_ladder() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let (spawner, spawns) = counting_spawner("exit 0");
        let sup = Supervisor::new(
            rt.clone(),
            door.clone(),
            Some(spawner),
            config(vec![Duration::from_millis(120)]),
        );
        let handle = sup.handle();
        let task = tokio::spawn(sup.run());

        // The spawned resident dies; the ladder arms (120ms out).
        wait_for("loop failed", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        let before = spawns.load(Ordering::SeqCst);

        // A resident attaches by hand: the door seats its (live) pid and the
        // attach handler signals the supervisor.
        door.record_pid(std::process::id());
        handle.on_attach(std::process::id());
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(
            spawns.load(Ordering::SeqCst),
            before,
            "no respawn over the attached resident"
        );
        assert_eq!(
            door.seat_pid(),
            Some(std::process::id()),
            "the attached resident keeps its seat"
        );
        task.abort();
    }

    /// The race half of the same anti-wedge: a respawn deadline that fires
    /// anyway (before the attach signal lands) still refuses to spawn over a
    /// live seat — probe before spawn.
    #[tokio::test]
    async fn respawn_deadline_never_spawns_over_a_live_seat() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        let (spawner, spawns) = counting_spawner("exit 0");
        let task = tokio::spawn(
            Supervisor::new(
                rt.clone(),
                door.clone(),
                Some(spawner),
                config(vec![Duration::from_millis(60)]),
            )
            .run(),
        );

        wait_for("loop failed", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        let before = spawns.load(Ordering::SeqCst);
        // Only the seat is taken (no attach signal): the deadline fires, the
        // pre-spawn probe finds a live pid, and no second loop spawns.
        door.record_pid(std::process::id());
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            spawns.load(Ordering::SeqCst),
            before,
            "the deadline aborted rather than spawning over a live seat"
        );
        task.abort();
    }

    /// An attached resident (no child handle) is probed by pid: a dead pid
    /// fails the loop.
    #[tokio::test]
    async fn attached_resident_death_is_detected_by_pid_probe() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        // As the attach door would: record a pid that is long dead.
        door.record_pid(4_000_000);
        rt.set_resident_expected();
        let task =
            tokio::spawn(Supervisor::new(rt.clone(), door.clone(), None, config(Vec::new())).run());

        wait_for("loop failed via probe", || {
            matches!(rt.loop_state(), LoopState::Failed { .. })
        })
        .await;
        assert!(door.seat_pid().is_none(), "the seat is freed");
        task.abort();
    }

    /// A completed assistant turn resets the ladder index: after the reset,
    /// the next death respawns on rung 0 again instead of climbing to a rung
    /// minutes out. (The ladder arithmetic itself is pinned above.)
    #[tokio::test]
    async fn completed_turn_resets_the_ladder() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rt = open_runtime(tmp.path());
        let door = ResidentDoor::new("tok");
        // Resident 1 dies instantly (→ respawn rung 0, attempts=1); resident
        // 2 lives half a second — long enough to complete a turn, resetting
        // attempts — then dies. Without the reset, that death arms rung 1
        // (10 minutes) and no third spawn ever arrives in test time.
        let count = Arc::new(AtomicU32::new(0));
        let counter = count.clone();
        let spawner: SpawnResident = Box::new(move || {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            let script = if n == 1 { "sleep 0.5" } else { "exit 0" };
            tokio::process::Command::new("sh")
                .args(["-c", script])
                .kill_on_drop(true)
                .spawn()
        });
        let task = tokio::spawn(
            Supervisor::new(
                rt.clone(),
                door,
                Some(spawner),
                config(vec![Duration::from_millis(30), Duration::from_secs(600)]),
            )
            .run(),
        );

        // While resident 2 lives, a turn completes: the ladder resets.
        wait_for("second spawn", || count.load(Ordering::SeqCst) >= 2).await;
        rt.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        rt.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        // Resident 2's death then respawns on rung 0 (30ms), not rung 1.
        wait_for("third spawn arrives fast (ladder reset)", || {
            count.load(Ordering::SeqCst) >= 3
        })
        .await;
        task.abort();
    }
}
