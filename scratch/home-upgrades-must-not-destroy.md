# Home upgrades preserve live work

## Problem

`lf install promote` currently treats every invocation as a new Home generation,
even when the downloaded release is byte-for-byte the installed release. The
2026-08-19 store records ten consecutive promotions of the same 0.12.8 source
revision from generation 143 through 153. Each one paused the keeper, stopped
the old generation, and relaunched enabled Work. The host wrapper is not a
second upgrade implementation: `studio/hosts/refresh-lf.sh` always delegates to
`scripts/install.py refresh`, which delegates to the downloaded candidate's
`lf install promote`. The candidate boundary must recognize the no-op.

Real upgrades also account for only part of the work they interrupt. The drain
tracks durable Runs, while interactive captures may be nested, unsupervised
AgentInvocations. `invocation_74115449e4bc4f5d8873a0de2f05f2c2` is the exact
counterexample: it remains `outcome=running`, `capture_status=capturing`, with no
`ended_at` or `supervising_run_id`. Its nested Exec has no terminal event because
SIGKILL bypassed cleanup, but its ancestor Exec receipt names an exact PID and
start time and that process is absent. `lf invocation status` cannot resolve an
unsupervised capture through the Run-only surface, and the current capture
reconciler waits for a terminal process event or a 48-hour age guard.

Task supervision then compounds the interruption. It can observe an
upgrade-stopped containment as an ordinary missing process, overwrite the typed
`HomeUpgrade` stop cause with `Recovery`, append `TaskEventKind::Failed`, and
create a `retry_of` chain. LOO-222 and LOO-223 show exactly that sequence after
Home-upgrade replacement Runs. Infrastructure churn therefore spends the same
three-attempt budget reserved for genuine work failure.

This work serves the infrastructure objective that ordinary work remain fast,
safe, and boring. It directly advances the Stability & Security proofs of zero
strands after process failure, release completion without manual repair, and
host/daemon recovery that preserves state and history.

## The demo

Run the host refresh twice at the same published release. The second log says
`0.12.x already installed; fleet untouched`; the runtime generation, live Run
IDs, and containment PIDs do not change.

During a real staged upgrade with no migration, keep a TUI manager and a Task
body live. The TUI keeps its PID, Invocation, Run authority, and terminal; the
Home replaces its control plane around it. A migration-bearing upgrade reports
that it is deferred while a non-resumable session is live instead of stopping
that session. Two consecutive upgrades leave the Task's work-failure recovery
count unchanged.

## Approach

### Make promotion a two-action decision

Add an internal `PromotionAction::{Noop, Upgrade}` decision after the candidate
has acquired the exclusive promotion lock and built its read-only preview, but
before it creates an upgrade receipt, recovery guard, generation, or fence.

`Noop` requires the complete requested installation to match the candidate:

- the installed CLI's own `install preflight --json` reports the same
  `CandidateIdentity` (build version, source revision, migration authority,
  source identity, and migration frontier);
- the installed daemon validates as the matching daemon candidate; and
- when this invocation requests the app, the installed app's bundled helper
  reports the same candidate and verdict.

Comparing the complete published identity prevents a reused version string or a
partially installed control-plane pair from being mistaken for a no-op. A
missing or mismatched requested target takes the ordinary upgrade path. `Noop`
prints one explicit line, creates no `home_upgrades` row, does not increment
`home_runtime_generations`, and never calls `pause_home`, `drain_active_runs`,
or reconciliation. It may still honor the explicit `--sync-skills` flag after
the no-op because skill synchronization does not stop or replace the fleet.

### Inventory every invocation before deciding what may stop

Extend `HomeUpgradeReceipt` with durable invocation receipts and materialize
them in a new `home_upgrade_invocations` table. The bridge JSON receipt carries
the same inventory so crash recovery works before and across candidate
migration. Once the real-upgrade decision is durable, install the active
promotion fence before taking this inventory. Run reservation already honors
that fence; make interactive invocation creation check it in the same store
transaction that creates the capture, so there is no check-then-launch gap.
Capture the inventory only after that admission barrier is closed and before
pausing the keeper.

Each receipt records:

- invocation ID, trace ID, Exec ID, surface, and optional supervising Run;
- the exact owning Exec receipt: PID, process start time, and the ancestor Exec
  ID that owns that PID;
- upgrade classification (`auto_resumable`, `preserve_required`, or
  `unprovable`);
- settlement (`pending`, `preserved`, `interrupted`, or `failed`); and
- an optional handoff artifact path.

Resolve nested ownership by walking `run_events.parent_process_id` within the
same trace until an exact `ExecProcessReceipt` is found. PID alone is never
evidence: liveness must match receipt PID, receipt start time, trace ID, and Exec
ID. A supervised invocation whose Run has ended is independently provable even
if its Exec receipt is missing. An unsupervised invocation with no exact owner
remains unprovable; an interactive unprovable capture blocks activation rather
than being guessed dead.

This snapshot is the sweep boundary. Invocations created by the new generation
are absent from it and cannot be accidentally settled. Immediately before
activation, rescan the old generation for open captures. A capture admitted
after the fence is an invariant violation: add it durably to the inventory and
repeat the decision until an entire scan is stable. This rescan is also the
rollout bridge for an already-running old CLI that predates the transactional
fence check.

### Preserve non-resumable interaction

Classify live `tui` and `ide` captures as `preserve_required`. They are excluded
from `request_upgrade_stop` and from both forced-drain signals. Replacing the
installed CLI and daemon does not replace an already-running process image, so
a schema-compatible promotion can activate the new control plane while the old
interactive owner, provider child, terminal, Invocation, and Run lease continue
unchanged. Reconciliation sees the preserved Run as already running and must
not launch a second writer for its Work.

A migration changes the safety result. Interactive capture opens the SQLite
ledger again when it records or finishes; an old binary intentionally refuses a
store whose migration frontier it does not recognize. Until interactive writes
have a generation-neutral protocol, `PromoteAndMigrate` is deferred before
pausing the Home whenever any `preserve_required` or `unprovable` owner is live.
The promotion log names each blocking Invocation and exact owner. Scheduled
refresh may retry after they finish. It never signals them and never consumes
Task recovery.

The upgrade retains the old content-addressed CLI and daemon bytes while a
preserved owner exists. No new attach or handoff abstraction is required merely
to let a process keep running. A handoff artifact is a recovery fallback for an
owner already found dead, not the normal upgrade path.

### Preserve infrastructure causality through Task supervision

Move absent-Run settlement behind one store transaction used by both the
upgrade coordinator and ordinary supervision. For a Home upgrade it atomically:

- preserves `StopCause::HomeUpgrade` on the Run;
- ends open Turns and Invocations as `Interrupted`, not `Failed`/`Unknown`; and
- ends the Run without emitting a Task work-failure event.

Expose an internal typed stop-cause read to Task/Project supervision. When
`reconcile_process_liveness` sees a `HomeUpgrade` stop, it defers to the upgrade
coordinator instead of calling ordinary `recover_run`. This removes the race in
which a Project runner rewrites the cause and appends `task process is missing`.

Only Runs captured by the matching active Home-upgrade receipt receive this
treatment. Relaunch an auto-resumable captured Run with the existing
`RunTrigger::HomeUpgrade`; it carries lineage without setting `retry_of` or
emitting `TaskEventKind::Failed`. A preserved interactive Run is not relaunched.
Once an upgrade completes, `RunTrigger::HomeUpgrade` alone is not an exemption:
a later process loss may be a genuine work failure and follows the ordinary
bounded `Failed` plus `RunTrigger::Recovery` path.

### Sweep the captured invocation boundary before completion

After the new keeper is healthy and enabled Work has reconciled, but before the
upgrade becomes `Completed`, sweep only the captured invocation inventory.

For each receipt:

- if its exact owner still exists, mark it `preserved` and leave the Invocation,
  Run, and process untouched;
- if its exact owner is absent, atomically end running Turns as interrupted,
  set `ended_at`, change `outcome=running` to `interrupted`, change
  `capture_status=capturing` to `interrupted` with the upgrade ID in
  `incomplete_reason`, and set supervised handback state to `interrupted`;
- remove an exact stale Exec receipt only after the invocation settlement
  commits; and
- leave already-terminal rows unchanged so recovery can repeat the sweep.

Use the same transaction primitive from absent-Run settlement instead of
keeping lifecycle SQL duplicated in `install.rs`. The ordinary reconciler also
uses the upgrade inventory to settle a preserved Invocation after its exact
owner later exits, without the generic 48-hour guard. A sweep write failure
keeps the upgrade in reconciliation/failure recovery; it cannot print
`Completed` with a captured dead owner still recorded as running. The
stale-receipt/unsupervised shape of `invocation_74115449...` is a permanent
fixture.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Can the host wrapper skip the upgrade itself? | No. `studio/hosts/refresh-lf.sh` is a thin caller and currently logs checkout movement, not installed artifact identity. Other installers call the same candidate boundary. | Decide no-op inside `lf install promote` under its exclusive lock; leave host-specific scripts out of the correctness path. |
| Is version text sufficient identity? | No. The runtime already records source revision and migration frontier, and CLI/daemon/app can drift independently. The 2026-08-19 repeats share the full identity and content-addressed CLI. | Require the full candidate plus every requested installed target to match. |
| Does the Run drain see every killed interaction? | No. The zombie is an unsupervised nested TUI AgentInvocation. Its durable Run join is empty, but its Exec ancestry reaches a stale exact process receipt. | Persist an invocation/Exec inventory in addition to `home_upgrade_work`. |
| Can the existing capture reconciler settle the zombie at upgrade completion? | No. SIGKILL emitted no terminal `run_events` row, and recent captures are intentionally protected by a 48-hour guard. | Reuse its terminal capture transition, but drive it from the upgrade's pre-drain exact-owner snapshot rather than age. |
| Why did TUI sessions reach forced drain? | The upgrade inventories Runs rather than Invocation surfaces, then applies one stop/TERM/KILL policy to every Run it sees. Interactive launch already uses `spawn`/`try_wait`; the missing distinction is resumability. | Inventory Invocations first and exclude preserve-required Runs from every stop and signal path. |
| Can an old interactive session coexist with the new control plane? | Yes when the store frontier is unchanged: replacing a symlink or daemon does not replace an already-running process image, and its existing lease remains valid. A migration is different because trace capture reopens and validates the ledger on later writes. | Preserve interaction across `Promote`; defer `PromoteAndMigrate` while a non-resumable owner is live. |
| Why did upgrade churn consume Task recovery? | Task supervision called ordinary `recover_run` after containment disappeared, overwrote the Home-upgrade cause, emitted `Failed`, then linked replacement Runs through `retry_of`. | Preserve the typed cause, defer matching upgrade-owned settlement, and relaunch only auto-resumable captured Runs with the existing Home-upgrade trigger. |
| How can the sweep avoid PID reuse and new-generation races? | Exec receipts already carry PID, start time, trace ID, and Exec ID; the upgrade knows its exact pre-drain invocation set. | Require all exact fields and sweep only snapshot IDs. Missing evidence is `unprovable`, never absent. |
| Can an invocation start between inventory and activation? | Run reservation already respects the active promotion fence, but interactive capture creation does not check it transactionally. An old CLI can also be alive during the first rollout. | Close admission before inventory, couple the fence check to capture creation, and require a stable old-generation rescan before activation. |
| Does preservation create another human-input protocol? | No. The session, terminal, Invocation, and Run continue as they were; no response is required. Durable Ask remains the only blocking human-input primitive. | Record preservation on the Home-upgrade receipt; do not mint a synthetic Run or Ask. |
| Can the inventory survive a migration or coordinator crash? | The upgrade already dual-writes a bridge JSON receipt before migration and a durable receipt when the schema is available. | Add invocation inventory to both representations and make every handoff/sweep transition idempotent. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Refuse every promotion while any Run or TUI is live | Safe but needlessly blocks schema-compatible releases even though Unix can keep old process images running. | Defer only migration-bearing upgrades that cannot prove safe coexistence; preserve sessions across ordinary promotions. |
| Put every interactive provider in tmux before allowing any upgrade | Makes later attachment explicit, but requires a new containment model and does not help sessions already live during the first rollout. | Process survival is already available without universal tmux. Detachable containment can be separate work if product behavior later requires attachment from another terminal. |
| Keep forced drain, exempt all upgrade-adjacent failures from the counter, then run `lf runs reconcile` | Small diff, but the human session is still destroyed, recent zombies remain age-guarded, and broad temporal exemptions can hide genuine work failures. | It treats symptoms after destroying work that could have remained live. |
| Compare only `lf --version` before promote | Meets the narrow happy path but silently accepts replaced release assets, mismatched daemons/apps, or a drifted migration frontier. | Full installed identity is already available and turns a plausible no-op into a proved no-op. |

## Key decisions

- A real Home upgrade is a durable transaction; an exact reinstall is not an
  upgrade and must create no generation or receipt.
- Non-resumable interaction survives schema-compatible activation. It is never
  stopped or signalled by promotion.
- A migration-bearing upgrade defers before pausing the Home when a live old
  writer cannot be proved compatible with the target frontier.
- An infrastructure interruption is evidence about the Home, not evidence that
  Task work failed. Matching captured Runs retain their Home-upgrade stop cause
  and use the existing Home-upgrade Run trigger without entering `retry_of`.
- Invocation cleanup follows exact process ownership. Timestamps and missing
  rows alone never authorize settlement.
- The promotion fence is an admission barrier as well as a Run-reservation
  barrier. Activation requires a stable inventory after the barrier closes.
- A handoff artifact is fallback evidence for an owner already found dead, not
  the normal path for a healthy session.
- The dangerous failure mode is a false liveness conclusion that interrupts an
  unrelated interactive process. Exact receipt matching, ancestor-cycle
  detection, pre-drain snapshotting, and fail-closed `unprovable` handling are
  release blockers, not polish.
- Do not change `studio`, build a generic host-upgrade platform, or replicate the
  mechanism to Cadenza in this Task. Loopflow must prove the shape first, matching
  the Wave bound against premature multi-product infrastructure.

## Scope

- In scope: installed-target identity and no-op promotion; Home-upgrade receipt
  schema and bridge persistence; Exec ancestry/liveness inventory; preserving
  interactive Runs across schema-compatible activation; deferring migrations
  around old writers; transactional interactive admission fencing; stable
  pre-activation rescans; typed upgrade-owned Run settlement; Task
  liveness/recovery classification; post-reconciliation invocation sweep; and
  focused plus staged behavioral proofs.
- Out of scope: edits to `studio/hosts/refresh-lf.sh`; moving a live TUI into
  tmux or reconstructing an unknown Claude/Codex session token; a
  generation-neutral trace-write protocol; a general unsupervised Run type;
  changing the three-attempt work-failure budget; Cadenza parity; and a generic
  multi-product deployment framework.

## Done when

Add a staged `tests/e2e/test_home_upgrade_survival.sh` (or an equivalent Rust
integration fixture that drives the real promotion boundary) and run it with
the focused Rust/Python suites:

```bash
cargo test -p loopflow lf::commands::install
cargo test -p loopflow ops::task
uv run pytest python/tests/test_shell_installer.py python/tests/test_release_automation.py
bash tests/e2e/test_home_upgrade_survival.sh
```

The staged proof must establish all four outcomes:

1. Promote one candidate twice while a sentinel Run and containment are live.
   The second invocation logs `already installed; fleet untouched`, creates no
   `home_upgrades` row, leaves the generation unchanged, and preserves the exact
   Run ID and PID.
2. Upgrade without a migration while a TUI fixture is live. The Home generation
   and control-plane PIDs advance while the TUI owner PID, provider child,
   Invocation, Run ID, lease, and terminal remain live and unchanged; the
   fixture observes no signal. After its natural exit, the Invocation and Turn
   settle and the upgrade receipt remains `preserved`. Repeat with a pending
   migration and prove promotion defers before pausing the keeper or signalling
   the TUI.
3. Kill one auto-resumable Task body through each of two distinct staged
   upgrades. SQL over `task_events` shows no corresponding
   `Failed { error: "task process is missing" }`; SQL over Runs shows each
   replacement has a Home-upgrade trigger and no new `retry_of` link. A
   subsequent genuine body failure still receives recovery attempt one.
4. Seed the exact `invocation_74115449...` class: an unsupervised nested TUI
   capture with a stale ancestor Exec receipt and no terminal process event. The
   completion sweep writes its handoff, settles the invocation/Turn as
   interrupted, removes only the exact stale receipt, and is idempotent on a
   second pass.

The proof advances these Wave/Project measures: seven consecutive days with
zero process-failure strands, four consecutive weekly releases with zero manual
repair, and host/daemon recovery that preserves state and history.

## Measure

Baseline from the 2026-08-19 Home evidence:

- ten consecutive identical 0.12.8 promotions advanced generations 143→153;
- at least two interactive TUI managers were interrupted without handoff;
- one known invocation remains `running/capturing` with an absent exact owner;
  and
- LOO-222/LOO-223 accumulated repeated `task process is missing` Failed events
  after Home-upgrade replacement Runs.

Track from `home_upgrades`, `home_upgrade_invocations`, `runs`, and
`task_events`:

- identical-install upgrades and generation increments: **0**;
- preserve-required owners receiving any upgrade signal: **0**;
- completed upgrades with captured dead-owner invocations still open: **0**;
- Home-upgrade missing-process observations emitted as Task `Failed`: **0**;
- change in a Task's work-failure recovery count across consecutive upgrades:
  **0**; and
- preserve-required invocations remaining live through compatible upgrades or
  explicitly blocking incompatible migrations: **100%**.
