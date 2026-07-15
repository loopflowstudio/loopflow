# Make Session bodies leased, progress-aware, and recoverable (W2-135)

One multi-PR Task, one stable worktree, one provider history. Absorbs W2-139.
W2-144's Now/Available/Roadmap views depend on this contract, not a parallel one.

## Thesis

A Wave, Project, or Task **Session** is durable intent. A runner, provider turn,
tmux session, PID, and process group are disposable **bodies** that temporarily
act for that intent. Users never manage bodies, but they must always see whether
durable work is advancing, why it is not, and what Loopflow will do next — the
same way on CLI, Mac, iOS, chat, prompts, and workers.

## What already exists (reconciled with #882/#886/#889/#893/#894 — all in main)

Base is `98e6882b` (#894). The durable-intent skeleton is already here; this Task
does not rebuild it, it makes it leased and progress-aware.

- `ChildSession` (`ops/child.rs`) is the **shared** Project+Task supervision spine:
  one `launch(intent)` with `LaunchIntent::{Supervisor, ExplicitResume}` already
  distinguishes "adopt/restart automatically" from "operator resumes review work."
- `ChildProcessGeneration { generation, pid, tmux_name, started_at }`
  (`child_session.rs`) — the durable per-body generation receipt, retained after
  exit so recovery advances the generation monotonically.
- `ChildExecutionContext { lf_bin, db_path, lf_home }` (#882) — the pinned
  execution context. A Session with `execution: None` (born before pinning)
  refuses to launch rather than let the calling process guess.
- `ChildCommandState::{Persisted, Claimed, Delivering, Accepted, Failed,
  Superseded, Uncertain}` — **already models the ambiguous-side-effect boundary**:
  `Delivering` = provider delivery began, `Uncertain` = process died after
  delivery began but before the outcome was recorded. Recovery must not blindly
  replay these.
- `supervisor_restart_bar` (`task/mod.rs`, `project_session/mod.rs`) — terminal
  intent wins (Completed/Abandoned), abandon-requested wins, open PR wins (the
  W2-129 fix). `AbandonIntent` makes "abandon requested" durable before a runner
  consumes it.
- `reserve_task_process` (`store/sqlite/child_sessions.rs`) — a status-CAS
  (`TransactionBehavior::Immediate`, `WHERE status = expected`). **This is the
  write lock the evidence names: the status column is being treated as the lease.**
- `Liveness { Observable, Unknowable }` + `is_gone(claims_process, process_alive)`
  (`lf/commands/waves.rs`) — binary liveness from `tmux has-session` (the #889
  probe fix). No progress dimension.
- Wire: `TaskRuntimeSnapshot`/`ProjectRuntimeSnapshot { status, process_alive, … }`
  (`lf/commands/waves.rs`) → Swift `WaveWorkMap.swift`. One collapsed `status`
  string + one `process_alive` bool.
- **One active supervisor loop, and it isn't the Sessions'.**
  `wave/supervisor.rs::Supervisor` runs the wave resident with a real event loop
  (respawn ladder, attach pid-probe every 10s, interrupt deadline). **Project and
  Task Sessions have no watcher loop at all** — "supervision" is the *passive*
  `supervisor_restart_bar` gate plus `claim_*_commands_or_stop` generation fencing,
  consulted only when someone runs `lf status` or a wake tries to relaunch. That
  asymmetry is why the 2026-07-14 bodies slept >4h: nothing looked between wakes.
- **Two liveness primitives that disagree.** `wave/registry.rs::process_alive`
  (pid probe, used by the resident supervisor) vs `engine/process.rs::
  tmux_session_exists` (`tmux has-session`, used by Session `Liveness`/attention).
  PR #889's "two paths in one snapshot disagreeing" lives here. The lease unifies
  them into one probe.
- Migrations: `store/migrations/MAJOR.MINOR.NNN_name.sql`, latest `0.11.002`;
  shipped migrations are immutable (`scripts/check_migrations.py`); ordering is the
  numeric `(major, minor, ordinal)` tuple. Next free is `0.11.003`.

## The five gaps this Task closes

| # | Gap | Evidence it caused |
|---|-----|--------------------|
| G1 | **Lease is implicit in the status column.** A generation exists but write authority is the status-CAS; a body that already holds a PID keeps writing regardless. | W2-132: two live Claude processes, one provider history + worktree — a status column treated as a write lock. |
| G2 | **No process-group ownership.** Bodies launch via tmux; the only reap is `tmux kill-session`. A detached grandchild (a hung `ssh`, a subprocess) survives. | Unbounded SSH held a turn (#893); supersession can't prove it reaped everything. |
| G3 | **No progress evidence.** Only `started_at` + `latest_event`. Cannot tell "alive but stuck 4h" from "alive and working." | 2026-07-14: three Claude bodies slept >4h while Tasks read `running` because a PID + tmux existed. |
| G4 | **Status collapses intent and observed body state** into one string. | Every surface must guess whether `running` means working or wedged. |
| G5 | **No progress-triggered recovery, and the uncertain boundary isn't gated at the supervision layer.** `Uncertain` exists for command delivery but nothing declares a live-but-stalled body dead, reaps it, and relaunches safely. | Blind replay across partial creation / restart / interrupt is unsafe (#882 refused to guess; #894 separated terminal history). |

## One-screen user model

You steer **intent**, never bodies. Every Session shows one **observation** of its
current body plus the evidence behind it:

- **Working** — a body holds the lease and made meaningful progress recently.
  Evidence: current step/command, progress age. Owner: the Session. Controls:
  steer, interrupt, stop.
- **Stalled** — a body holds the lease and is alive but has made no meaningful
  progress past its deadline. Evidence: current command, progress age, deadline.
  Owner: Loopflow (will recover). Controls: extend, interrupt, stop.
- **Recovering** — Loopflow revoked a lost/stalled lease, reaped its process
  group, and is starting generation+1 on the same Session. Evidence: prior
  outcome, new generation. Owner: Loopflow. Controls: stop.
- **NeedsInput** — a body was lost during an uncertain external side effect, or
  hit a decision. Loopflow will **not** replay. Evidence: the exact command and
  why it's uncertain. Owner: human. Controls: decide/resume, abandon.
- **Stopped** — no live body, intent not terminal; a wake will adopt or start
  one. Evidence: last outcome. Owner: Session/Wave. Controls: resume.
- **Failed** — the last body failed and the flow itself failed (not a retryable
  attempt). Evidence: termination reason. Owner: human. Controls: resume, abandon.
- **Terminal** — Completed / Abandoned. Never restarts. Owner: none.
- **Unobservable** — this machine cannot tell (no tmux). Never asserted as gone.
  Owner: unknown. Controls: none automatic.

Humans see occasional clear updates and the actionable state. Heartbeats and raw
command evidence stay in the append-only audit, not the human feed (contract #8).

## Shared data types

Extend the existing generation receipt into a **write lease**; add a derived
**observation** projection. Keep one implementation across Wave/Project/Task.

```rust
// child_session.rs — extends ChildProcessGeneration into the lease.
pub struct BodyLease {
    pub generation: u32,
    pub lease_token: LeaseToken,          // new: opaque per-generation write token
    pub pid: Option<u32>,
    pub pgid: Option<u32>,                // new: process group captured via setsid
    pub tmux_name: String,
    pub provider_session: Option<String>, // provider turn identity
    pub execution: ChildExecutionContext, // pinned (was already on the Session)
    pub started_at: OffsetDateTime,
    pub heartbeat_at: OffsetDateTime,     // new: body is alive
    pub activity_at: OffsetDateTime,      // new: any provider event
    pub progress_at: OffsetDateTime,      // new: last durable mutation / step advance
    pub deadline: Option<OffsetDateTime>, // new: bounded or explicitly extended
    pub outcome: Option<BodyOutcome>,     // terminal outcome of THIS generation
}

// Derived, NOT stored — a projection over Session status + lease + a Clock.
// Same discipline as AttemptFailurePresentation. No second monitor store.
pub enum BodyObservation {
    Working    { step: String, progress_age: Duration },
    Stalled    { command: String, progress_age: Duration, deadline: OffsetDateTime },
    Recovering { prior: BodyOutcome, generation: u32 },
    NeedsInput { command: String, why: UncertainReason },
    Stopped    { last: Option<BodyOutcome> },
    Failed     { reason: String },
    Terminal   { status: &'static str },
    Unobservable,
}
```

`BodyObservation` carries **evidence, next owner, and legal controls** for every
variant (contract #5). Liveness (does a body exist) and progress (is it advancing)
are separate inputs; `Working` vs `Stalled` is exactly their difference (G3).

Wire: replace the `TaskRuntimeSnapshot`/`ProjectRuntimeSnapshot`
`{ status, process_alive }` pair with `{ status /*durable intent*/, observation,
evidence, next_owner, controls }`. `status` stays (durable intent); `observation`
is the observed body. DTO rule holds: every new field required or explicit
`Optional`, mirrored in Swift `WaveWorkMap.swift`, covered by a
`tests/fixtures/dto/` round-trip fixture.

## State-transition table (durable intent × observed body → observation)

| Durable status | Lease present? | Alive? | Progress fresh? | Uncertain cmd? | → Observation | Next owner |
|----------------|----------------|--------|-----------------|----------------|---------------|-----------|
| Running/Starting | yes | yes | yes | no | Working | Session |
| Running | yes | yes | **no, past deadline** | no | **Stalled** | Loopflow |
| Running | yes | no | — | **yes** | **NeedsInput** | human |
| Running | yes | no | — | no | Recovering→gen+1 | Loopflow |
| Running | revoked/stale gen | — | — | — | (stale body exits read-only) | — |
| Waiting/Blocked | no | — | — | — | Stopped | Session/Wave |
| Failed (flow) | no | — | — | — | Failed | human |
| Completed/Abandoned | — | — | — | — | Terminal | none |
| any | — | tmux absent | — | — | Unobservable | unknown |

Terminal intent dominates every row (contract #6). `supervisor_restart_bar`
already enforces this and open-PR review; the observation projection must agree
with it, never contradict it.

## Recovery policy

**Who notices a stall.** Contract #1 forbids a Task-only watchdog. There is no
Session-level loop today, so the recovery driver is the **existing parent loop**:
a Project loop wake evaluates its child Tasks' leases (progress-aware), and the
Wave resident supervisor — already an active loop — evaluates Project leases via
the same `BodyObservation`. No new watcher process; the lease `deadline` is what a
wake enforces. If the parent's wake cadence proves too coarse to catch a stall
inside its lease window, that is the "project-loop caps" open fork — tighten the
lease deadline, don't add a watchdog.

A **replay-safe boundary** = a flow step boundary with (a) no in-flight
`Delivering` command and (b) no `Uncertain` external side effect on the latest
generation. Recovery is automatic only there.

1. **Stalled** (alive, progress past deadline): revoke lease → reap process group
   (`kill -TERM -<pgid>`, escalate) → confirm dead → start gen+1 on the same
   Session/worktree/provider history/pinned context → **Recovering**. Sampling
   must never show two live writers.
2. **Lost during an uncertain side effect** (`Delivering`/`Uncertain`): →
   **NeedsInput**, command + reason preserved, no replay. `mark_stale_child_
   deliveries_uncertain` already exists — wire it to the observation, not a retry.
3. **Terminal / abandon-requested / open-PR** → never restart (existing
   `supervisor_restart_bar`).
4. **Host or process restart** → adopt a healthy body via lease token match, or
   converge through the same state machine to Stopped/Recovering. No parallel
   intent is ever created as recovery (contract #7).

## Migration strategy

Additive columns on `task_sessions` and `project_sessions` (lease_token, pgid,
provider_session, heartbeat_at, activity_at, progress_at, deadline) — a superset
of today's `process_generation`/`process_pid`/`process_tmux_name`/
`process_started_at`. New migration `0.11.003_body_lease.sql`.

**Shared-lfdb hazard (wave memory):** migration numbers collide across branches on
one `~/.lf/lfdb`; version-prefixed names apply but inter-order is undefined. Keep
every ALTER additive and idempotent-safe; do **not** edit a historical migration
in place. If product/intelligence both mint `0.11.003`, distinct version strings
still apply — but prefer the next free number and, per Jack's note, dogfood
against `LF_HOME=~/.lf-dev` so in-flight schema can't corrupt the real ledger.

## Deterministic tests

- **Fake provider, alive, no progress** → `Stalled` under an injected `Clock` once
  `progress_age > deadline`; observation names step/command + progress age.
- **Reap group**: launch a body that forks a grandchild; supersession kills the
  whole `pgid`; grandchild is gone.
- **≤1 writer**: sample the lease under concurrent adopt/relaunch; only the
  current `(generation, lease_token)` writes; stale gen sees revoked lease + exits.
- **Uncertain → NeedsInput**: kill a body mid-`Delivering`; recovery yields
  NeedsInput, not a replay.
- **Terminal never restarts**: Completed/Abandoned/open-PR bars gen+1.
- **Adopt healthy body**: a wake over a live, progressing lease does not spawn.
- **DTO parity**: Rust + Swift decode one fixture with identical observation,
  evidence, category, reason, next owner, controls.

## Serial PR sequence (one worktree, ordered branches)

**PR1 — Shared supervision + wire model.** Land `BodyLease` (extending
`ChildProcessGeneration`) + `BodyObservation` projection + the state-transition
table in code, with bodies writing `heartbeat_at`/`activity_at`/`progress_at` on
step/provider boundaries. Derive the observation in the work-map producer
(`lf/commands/waves.rs`), replace `{status, process_alive}` on the runtime
snapshots, mirror in `WaveWorkMap.swift`, add the DTO fixture. Both the wave
`Supervisor` and `ops/child.rs` express the shared types.
*Demo:* `lf status --json` shows `observation` + progress age; a running task
reads Working with a live progress age, a wedged one reads Stalled. Migration
`0.11.003`. **This is the decisive first vertical slice.** Absorbs W2-139.

**PR2 — Atomic write-lease + process-group ownership.** Replace the status-CAS
with an explicit `(generation, lease_token)` lease; capture `pgid` via `setsid`
at launch; supersession atomically revokes the lease, reaps the whole process
group, confirms death, then starts gen+1; a stale generation detects the revoked
lease and exits read-only. Sampling test proves ≤1 writer.

**PR3 — Progress lease + safe recovery.** The **parent loop** (Project wake for
Tasks, Wave resident supervisor for Projects — not a new watchdog) declares a body
Stalled past its progress deadline, recovers it at a replay-safe boundary; an
uncertain side effect becomes NeedsInput with no replay; terminal intent never
restarts. Fake-provider deterministic-clock stall→recover test. Unify the pid vs
tmux liveness probes into the single lease probe here.

**PR4 — Cross-surface status + controls.** W2-144's Now/Available/Roadmap
consumers read this observation contract; Mac/iOS/chat render category + reason +
next owner + controls; human feed gets occasional clear updates while heartbeats
stay in audit.

**PR5 — Deterministic end-to-end dogfood.** One intentional real provider stall,
detected within its lease and recovered on the same Session before a human notices
from wall-clock time. Full Rust + Swift + migration + smoke gates.

Adjust boundaries if the code proves a simpler sequence; preserve the single
product model across Wave/Project/Task.

## Pursue target (this phase leaves)

Build **PR1**: introduce `BodyLease` + `BodyObservation`, have the launch/step
paths stamp heartbeat/activity/progress, derive the observation in
`lf/commands/waves.rs`, reshape the runtime-snapshot wire + Swift mirror + DTO
fixture, add migration `0.11.003`. Verify with `cargo test -p loopflow`, the DTO
fixture test, the Swift model test, and `lf status --json` on a live task showing
`observation` with a progress age.
