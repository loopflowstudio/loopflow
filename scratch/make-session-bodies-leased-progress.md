# Make Session bodies leased, progress-aware, and recoverable (W2-135)

One Task Session, one stable worktree, ordered serial PRs. W2-135 absorbs W2-139;
W2-144 and its Now/Available/Roadmap consumers depend on this contract rather
than defining another liveness model.

## Directive v8 and the current serial boundary

Directive v8 was acknowledged on 2026-07-14. Its requested provider-handoff
slice is already merged as PR #901 (`53a7209d`):

```bash
lf task resume W2-135 --model codex --reason "Claude quota exhausted"
lf project resume loopflow-api --model codex --reason "Claude quota exhausted"
```

That PR preserves Session identity, directive, worktree, PR sequence, and pinned
execution context; clears an incompatible provider transcript; writes a typed
`BodyHandedOff` event; exposes generation/agent/provider in status; rejects a
live writer; and preserves terminal, abandonment, and open-PR supervisor bars.
Plain resume continues compatible provider history.

The runner rotated to serial PR 3 after #901 merged. This design therefore does
not rebuild provider handoff. PR 3 completes the authority that #901 named but
did not yet fully enforce: a token-fenced write lease and bounded whole-body
reaping shared by Project and Task Sessions. The installed `lf` used by this
already-running Session may predate #901 because its execution context is
pinned; source and test behavior at `53a7209d` are authoritative for this branch.

## Product thesis

A Wave, Project, or Task Session is durable intent. A runner, provider turn,
tmux session, PID, process group, and provider transcript handle are disposable
bodies that temporarily act for it. Users steer intent, never bodies, but can
always see whether the current body is advancing, why it is not, who acts next,
and which controls are legal.

## Current implementation audit

The durable skeleton is real. Keep it and make it true under concurrency.

- PR #882 pinned `ChildExecutionContext { lf_bin, db_path, lf_home }`. Legacy
  Sessions without it refuse relaunch rather than guessing a binary or store.
- PR #889 made tmux availability and `tmux has-session` truthful for Session
  snapshots. Wave residency still has its own PID supervision; do not reuse that
  as a second Project/Task liveness model.
- PR #893 bounded remote SSH probes. It proves a live process is not progress:
  the current command and deadline must be first-class evidence.
- PR #894 separated terminal Project history from its successor. Recovery must
  preserve the existing Session unless its durable intent is terminal; it must
  never manufacture a successor as a process-recovery mechanism.
- PR #898 added the shared, clock-free `BodyObservation` projection:
  `Working`, `Stalled`, `Recovering`, `NeedsInput`, `Stopped`, `Failed`,
  `Terminal`, and `Unobservable`, with reason, next owner, legal controls,
  progress age, deadline, and step. It is not yet carried by Task/Project status.
- PR #901 added provider handoff and reframed
  `ChildProcessGeneration.generation` as a fencing token. The handoff mutation is
  atomic and rejects `status.is_process_active()`, but body writes are not fully
  fenced by generation.

The remaining failure paths are concrete:

1. `reserve_task_process` and `reserve_project_process` CAS only on `status`.
   Two launchers starting from the same inactive row can be serialized, but the
   status column remains the authority.
2. Active runners call unrestricted `update_task_session` /
   `update_project_session`. A stale generation can overwrite a successor after
   it has lost the generation race; only command claim and stop boundaries check
   generation today.
3. The hidden `__task` / `__project` runner receives a generation but no opaque
   lease token. Ambient child `lf` commands inherit Session identity without a
   store-backed proof that their body still owns it.
4. `ChildProcessGeneration` records generation, optional PID, tmux name, and
   start time. It does not record the acting provider, process group, lease
   state, or terminal outcome for that body.
5. Session reconciliation probes tmux and marks a missing body Waiting/Failed.
   It does not revoke an authority, reap a process tree, or prove a successor
   starts only after the old writer is gone.
6. Task and Project runners poll commands every 200 ms but emit no heartbeat or
   current step/command lease evidence. Provider events distinguish text,
   commands, tools, and turn boundaries, so activity and meaningful progress can
   be separated without inventing provider-specific status.
7. The Wave resident supervisor has an active respawn loop. Project and Task
   supervision is passive and runs when status, a parent pass, or a control
   command reconciles it. The Wave listener's `ResidentDoor` records only a seat
   PID and owns a respawn ladder, not a body generation receipt. Add no Task-only
   watchdog; later recovery must use one shared observation and lease vocabulary
   at those existing wake points, with the listener remaining Wave's driver.

## One-screen user model

| Body state | Evidence shown | Next owner | Legal controls |
|---|---|---|---|
| Working | generation/provider, step or command, recent progress | Session | steer, interrupt, stop |
| Stalled | live body, current command, progress age past deadline | Loopflow | extend, interrupt, stop |
| Recovering | revoked generation/outcome, successor generation | Loopflow | stop |
| NeedsInput | uncertain command/side effect, decision, or submitted review; automatic replay refused | human | decide/resume, abandon |
| Stopped | no live body, non-terminal intent, last outcome | Loopflow | resume, stop |
| Failed | body/flow failure and termination reason | human | resume, abandon |
| Terminal | completed or abandoned durable intent | nobody | none |
| Unobservable | this machine cannot prove liveness | unknown | no automatic control |

Durable Session status and observed body state remain separate. Terminal intent
dominates every observation. Heartbeats and raw command evidence stay in audit;
human feeds receive only state changes and occasional progress summaries.

## Shared types

Keep one shared internal lease contract and one public generation projection for
both `TaskSession` and `ProjectSession`. The opaque token is a local write
capability: it must never serialize through `TaskSessionSnapshot` /
`ProjectSessionSnapshot`, appear in events, use derived `Debug`, or enter logs.
Store it in its own database column and return it only in the private launch
reservation passed to the runner.

```rust
pub struct ChildLeaseToken(String); // private; custom redacted Debug, no serde

pub(crate) struct ChildWriteLease {
    token: ChildLeaseToken,
    generation: ChildProcessGeneration,
}

// Safe status/audit evidence. It deliberately has no token.
pub struct ChildProcessGeneration {
    pub generation: u32,
    pub pid: Option<u32>,
    pub process_group_id: Option<u32>,
    pub tmux_name: String,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub started_at: OffsetDateTime,
    pub state: ChildLeaseState,
    pub outcome: Option<ChildBodyOutcome>,
}

pub enum ChildLeaseState { Legacy, Reserved, Active, Revoked, Finished }

pub enum ChildBodyOutcome {
    Completed,
    Interrupted,
    Failed { reason: String },
    Lost { reason: String },
    Superseded { reason: String },
    LegacyStopped { reason: String },
}
```

The Session retains the immutable `ChildExecutionContext`; the generation
receipt refers to that one pinned context rather than duplicating paths that can
drift. The receipt snapshots agent/provider/provider-session because Session
agent selection may change before a later generation and audit must still say
which body actually ran. The relational store compares the separately held
token in every body-owned mutation, then returns only redacted generation
evidence to status callers.

Add typed `BodyLeaseChanged` Task/Project events carrying generation, state,
outcome, and provider evidence. They are the append-only history for prior body
generations; the Session row is only the current receipt. `BodyHandedOff` remains
the separate operator decision selecting the next agent.

PR 3 adds only authority/lifecycle fields. Heartbeat, activity, progress,
deadline, step, and command join the same receipt in PR 4, when their semantics
and recovery behavior are implemented; do not land unused schema or dead fields.

## Write authority

The active authority is the tuple `(session_id, generation, lease_token)`, not
`status` and not PID existence.

1. A launch transaction verifies durable restart bars, advances generation,
   creates a random token, stores a `Reserved` receipt, changes status to
   Starting, and appends the lease event atomically.
2. The token and generation are passed to `__task` / `__project` and exported as
   `LF_TASK_LEASE_TOKEN` / `LF_PROJECT_LEASE_TOKEN` alongside the existing
   Session and generation environment.
3. Before starting a provider, the runner exchanges `Reserved` for `Active` and
   records its own PID/tmux evidence with a token CAS. Whenever the harness
   starts or replaces its provider process, it exposes that current process
   group and the runner records it through the same token CAS.
4. Every body-owned store mutation uses a lease-checked operation. At minimum:
   Session state/provider transcript updates, directive incorporation, command
   claims/delivery outcomes, flow-boundary stop, progress/failure/completion
   events, PR rotation/publication invoked from the body, and unhandled-failure
   recording. A stale token changes zero rows and returns a typed `LeaseRevoked`
   result; the runner stops its harness and exits without another write.
5. Human/Wave/Project controls may append commands and durable intent, but they
   do not impersonate the body lease. Split control-plane mutations from
   body-owned mutations instead of teaching unrestricted full-row updates about
   both.
6. An ambient Session command validates its generation/token before any mutating
   operation. Read-only status/audit commands remain available to stale bodies.

Generation stays monotonic and human-readable. The random token prevents a
stale process from gaining authority by knowing or guessing the next integer.

## Revoke and reap

PR 3 supplies one shared `revoke_and_reap_child_body` primitive for later
recovery. It is not a new supervisor and it does not decide whether replay is
safe.

1. Atomically change the matching Active/Reserved lease to Revoked with a typed
   outcome. From that commit onward all body writes fail their token CAS.
2. Stop the provider through the runner's existing interrupt cleanup, kill the
   tmux session, signal the recorded provider/body process group with TERM, and
   escalate to KILL after a short bounded grace period.
3. Confirm both tmux absence and process-group absence. Do not reserve generation
   + 1 until confirmation. A failed reap leaves the Session Recovering/Stopped
   with the old lease revoked; it never launches a concurrent writer.
4. Mark the receipt Finished only after the body is proven gone. Preserve the
   revoke reason and outcome in the append-only event.

Harnesses already have different cleanup details: Codex owns an app-server
process group, Claude owns a CLI child per turn, and OpenCode owns a server
lifecycle. Expose their current provider process-group identity through the
shared harness interface and refresh it whenever that process changes. Tmux plus
runner PID remain the outer body identity; reaping checks both layers. Do not add
a provider-specific supervisor beside this path.

The existing `resume --model` behavior continues to reject a live writer in
this slice. Automatic or operator supersession of a live body is enabled only
after PR 4 can classify the in-flight command as replay-safe or uncertain.

## State transitions

| Current lease | Event | Transaction/result |
|---|---|---|
| none/Finished/Revoked | legal launch | reserve generation + 1 with new token |
| Reserved | matching runner starts | Active; record PID/group/provider |
| Active | matching body write | accept; lease remains Active |
| Active | stale generation/token write | reject `LeaseRevoked`; no mutation |
| Active/Reserved | revoke requested | Revoked first, then bounded reap |
| Revoked | old body exits | Finished with preserved outcome |
| Revoked | successor requested before reap proof | reject; no generation + 1 |
| Active | second launch races | adopt current healthy lease; no new body |
| any | completed/abandoned/abandon-requested intent | no launch |
| inactive + open/publishing Task PR | supervisor wake | no launch; explicit review resume remains legal |

PR 4 adds `Working -> Stalled -> Recovering` and `lost + uncertain ->
NeedsInput` on top of these mechanical lease states. PR 3 must not infer replay
safety from process death alone.

## Recovery policy for the next slice

Automatic recovery is legal only at a proven replay-safe boundary: no command in
`Delivering` or `Uncertain`, no active external side effect, terminal/review bars
clear, and pinned execution context present.

- Live + fresh progress: adopt the current lease.
- Live + progress deadline exceeded: Stalled; revoke/reap; recover the same
  Session as generation + 1 only if replay-safe.
- Lost + replay-safe: revoke/reap evidence, then recover the same Session,
  worktree, provider history, directive, and PR.
- Lost during `Delivering` or an external command: NeedsInput with exact command
  evidence; no replay.
- Completed, abandoned, abandon-requested, publishing/open PR under supervisor
  control: never restart.
- Unobservable host: never assert the body is dead and never auto-recover.

The Project runner evaluates Task observations at its existing child-observation
boundary. The Wave resident evaluates Project observations through the same
operation. A host restart converges through this state machine: validate/adopt a
matching healthy lease or revoke the lost lease and apply the same recovery
policy. No successor Session and no second monitor store.

## Serial delivery

### Landed: PR 1 / #898 — shared user state model

Added `BodyObservation`, intent coarsening, evidence/owner/control vocabulary,
and deterministic projection tests. Wire integration remained deliberately
deferred while W2-134 owned live-turn DTO changes.

### Landed: PR 2 / #901 — replaceable provider body

Added audited Task/Project `resume --model`, compatible-history default resume,
generation/provider status, incompatible transcript clearing, and restart bars.
Proved failed Claude -> Codex continuation on the same Task/PR without SQLite
edits.

### Current: PR 3 — explicit write lease and whole-body ownership

- Add a private `ChildLeaseToken`/`ChildWriteLease`; add lease state/outcome,
  provider snapshot, and process group only to the safe shared generation
  evidence. Prove the token is absent from JSON, events, Debug, and errors.
- Add migration `0.11.003_child_body_lease.sql`; update both Task and Project
  rows through the same public store contract. The migration gives every
  historical generation a random token. Inactive history becomes Finished;
  process-active rows become Legacy because their already-running body cannot
  know the new token. New code may adopt neither nor launch over it: explicit
  reconciliation first reaps the legacy tmux/provider group, records
  `LegacyStopped`, then reserves a token-aware successor. This is the production
  database cutover, not a permanent dual protocol. Only new bodies may be Active.
- Replace active-runner unrestricted updates with generation+token CAS methods.
- Pass/validate the token at hidden runner and ambient command boundaries.
- Add typed lease lifecycle events and shared revoke/reap mechanics.
- Keep live `resume --model` rejection until replay policy lands.

Proof: concurrent launch yields one active token; stale generation writes and
completion reports are rejected; revocation kills a fake runner plus forked
grandchild and proves both absent before generation + 1; Task and Project tests
exercise the same contract; terminal/open-PR bars still hold.

### PR 4 — progress evidence, wire, and safe recovery

Add heartbeat/activity/progress/deadline/current step/current command to the
lease. Provider events update activity; only durable mutation, flow advance,
directive incorporation, or PR/state change updates meaningful progress. Feed
the shared `BodyObservation` into Wave/Task/Project status and the machine-wide
work wire after W2-134's delta contract. Adapt the Wave listener's resident seat
to the same generation/outcome/observation vocabulary while retaining its
existing supervisor loop and journal as the driver and audit. At existing parent
wake points, adopt healthy leases, recover replay-safe lost/stalled bodies, and
map uncertain delivery to NeedsInput. Use a deterministic clock and fake
provider that remains alive but never progresses.

### PR 5 — cross-surface status and controls

Mirror the required DTO in Swift; CLI, Mac, iOS, chat, and W2-144 consumers show
the same category, evidence, reason, owner, and controls. Human feeds suppress
heartbeat noise. No surface-owned lifecycle rules.

### PR 6 — deterministic and real dogfood

Run one intentional real provider stall. Detect and recover it on the same
Session before wall-clock inspection. Exercise process restart and host restart,
safe and uncertain side effects, abandonment, completion/submission, and
provider handoff. Run full Rust, Swift, migration, DTO, and smoke gates. Complete
W2-135 only when the broader proof holds.

## Pursue target for serial PR 3

Implement only the explicit lease/reap slice above. Start at
`child_session.rs`, `store/child_sessions.rs`, and
`store/sqlite/child_sessions.rs`; then thread the token through
`ops/{child,task,project}.rs`, hidden CLI runners, both runners, and the harness
process identity. Update docs only for behavior this PR makes true.

Verification target:

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow child_session
cargo test -p loopflow child_sessions
cargo test -p loopflow task_process_resume_reserves_each_session_generation_once
cargo test -p loopflow
uv run python scripts/check_migrations.py
```

Add focused test names for stale-token rejection, Task/Project parity, bounded
process-group reap, and no successor-before-reap; include them in the targeted
commands as their final module names settle. No production progress watchdog,
wire reshaping, Swift UI, or automatic replay belongs in this PR.
