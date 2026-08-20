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

During a real staged upgrade, keep a TUI manager and a Task body live. The log
prints a private handoff artifact before the TUI receives graceful termination,
the AgentInvocation settles as interrupted, and two consecutive upgrades
produce typed Home-upgrade restart events while the Task's work-failure recovery
count remains unchanged.

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

### Inventory every invocation before a real drain

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
- drain classification (`auto_resumable`, `handoff_required`, or
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
after the fence is an invariant violation: add it durably to the inventory,
drain it by the same classification, and block activation until an entire scan
is stable. This rescan is also the rollout bridge for an already-running old
CLI that predates the transactional fence check.

### Drain non-resumable interaction before activation

Classify `tui` and `ide` captures without a durable attach-capable containment
as `handoff_required`. Before signalling their exact, deduplicated owning Exec:

1. Atomically write
   `~/.lf/upgrades/<upgrade-id>/handoffs/<invocation-id>.md` with mode `0600`.
   The artifact names the Work when one exists, provider, surface, worktree,
   skill, trace artifact, interruption cause, and the ordinary command for
   starting a fresh session. It contains no prompt body, provider credential,
   or raw resume token.
2. Persist the path in the upgrade receipt and print
   `handoff <invocation-id>: <path>` to the promotion log.
3. Send SIGTERM to the exact verified owning Exec once. The existing interrupt
   hook terminates its provider child; no interactive owner is eligible for the
   later SIGKILL fallback.

Replace interactive `Command::status()` waiting with a controlled
`spawn`/`try_wait` loop. New TUI wrappers poll the active upgrade fence, finish
their capture as interrupted, preserve the handoff, terminate their child, and
return a typed Home-upgrade exit rather than a generic launcher failure. The
candidate-side SIGTERM remains necessary for the first deployment, whose live
wrappers do not yet know how to poll the fence.

If a handoff-required owner remains live after the drain grace, fail before
artifact activation. Preserve its handoff and the old Home rather than escalate
to SIGKILL. Auto-resumable Run containment retains the existing bounded
SIGTERM/SIGKILL fallback because its durable Work can be relaunched. An
attach-capable provider-only containment may be marked `preserved`; an old `lf`
process may not survive against a migrated store.

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

If a replacement Run whose trigger is `RunTrigger::HomeUpgrade` later disappears
without its own atomic body-failure receipt, treat the missing receipt as an
infrastructure restart. Append a concrete
`TaskEventKind::HomeUpgradeRestarted { upgrade_id, prior_run_id }` and relaunch
with `RunTrigger::HomeUpgrade`, whose `prior_run_id` carries lineage but whose
`retry_of` remains `None`. Do not append `TaskEventKind::Failed` and do not count
the event as durable work progress. Repeated failure to restore the replacement
marks the upgrade Work receipt failed and surfaces Home attention; it never
converts into Task abandonment. Provider errors and explicit body failures keep
the existing `Failed` plus `RunTrigger::Recovery` path and remain bounded at
three.

### Sweep the captured invocation boundary before completion

After the new keeper is healthy and enabled Work has reconciled, but before the
upgrade becomes `Completed`, sweep only the captured invocation inventory.

For each receipt:

- if its exact owner still exists, mark it `preserved` and leave the invocation
  untouched;
- if its exact owner is absent, atomically end running Turns as interrupted,
  set `ended_at`, change `outcome=running` to `interrupted`, change
  `capture_status=capturing` to `interrupted` with the upgrade ID in
  `incomplete_reason`, and set supervised handback state to `interrupted`;
- remove an exact stale Exec receipt only after the invocation settlement
  commits; and
- leave already-terminal rows unchanged so recovery can repeat the sweep.

Use the same transaction primitive from absent-Run settlement instead of
keeping lifecycle SQL duplicated in `install.rs`. A sweep write failure keeps
the upgrade in reconciliation/failure recovery; it cannot print `Completed`
with zombie records outstanding. The stale-receipt/unsupervised shape of
`invocation_74115449...` is a permanent fixture.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|------------------|
| Can the host wrapper skip the upgrade itself? | No. `studio/hosts/refresh-lf.sh` is a thin caller and currently logs checkout movement, not installed artifact identity. Other installers call the same candidate boundary. | Decide no-op inside `lf install promote` under its exclusive lock; leave host-specific scripts out of the correctness path. |
| Is version text sufficient identity? | No. The runtime already records source revision and migration frontier, and CLI/daemon/app can drift independently. The 2026-08-19 repeats share the full identity and content-addressed CLI. | Require the full candidate plus every requested installed target to match. |
| Does the Run drain see every killed interaction? | No. The zombie is an unsupervised nested TUI AgentInvocation. Its durable Run join is empty, but its Exec ancestry reaches a stale exact process receipt. | Persist an invocation/Exec inventory in addition to `home_upgrade_work`. |
| Can the existing capture reconciler settle the zombie at upgrade completion? | No. SIGKILL emitted no terminal `run_events` row, and recent captures are intentionally protected by a 48-hour guard. | Reuse its terminal capture transition, but drive it from the upgrade's pre-drain exact-owner snapshot rather than age. |
| Why do TUI sessions reach forced drain? | Interactive launch blocks in `Command::status()` and never polls `RunControl::Quiesce` or the Home fence. | Add a controlled wait loop; keep candidate-side SIGTERM as the rollout bridge for older wrappers. |
| Can graceful drain fall back to SIGKILL? | Not for a non-resumable interaction: that recreates the incident. | Handoff-required liveness aborts activation. Only auto-resumable containment remains forceable. |
| Why did upgrade churn consume Task recovery? | Task supervision called ordinary `recover_run` after containment disappeared, overwrote the Home-upgrade cause, emitted `Failed`, then linked replacement Runs through `retry_of`. | Preserve typed cause, defer upgrade-owned settlement, and use Home-upgrade triggers/events for receipt-less infrastructure loss. |
| How can the sweep avoid PID reuse and new-generation races? | Exec receipts already carry PID, start time, trace ID, and Exec ID; the upgrade knows its exact pre-drain invocation set. | Require all exact fields and sweep only snapshot IDs. Missing evidence is `unprovable`, never absent. |
| Can an invocation start between inventory and activation? | Run reservation already respects the active promotion fence, but interactive capture creation does not check it transactionally. An old CLI can also be alive during the first rollout. | Close admission before inventory, couple the fence check to capture creation, and require a stable old-generation rescan before activation. |
| Does the handoff create another human-input protocol? | No response is required; it is an immutable interruption receipt and restart guide. Durable Ask remains the only blocking human-input primitive. | Store the artifact on the Home-upgrade receipt instead of minting a synthetic Run or Ask. |
| Can the inventory survive a migration or coordinator crash? | The upgrade already dual-writes a bridge JSON receipt before migration and a durable receipt when the schema is available. | Add invocation inventory to both representations and make every handoff/sweep transition idempotent. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Refuse every promotion while any Run or TUI is live | Minimal mutation risk, but scheduled upgrades can remain blocked indefinitely and no recovery is exercised. | It defeats headless release maintenance and still leaves stale invocation rows after an external kill. |
| Put every interactive provider in tmux and let it survive | Excellent interaction continuity once universal, but it needs a new unsupervised-containment model and must prove no old `lf` process can touch the migrated store. | This is as hard as repairing the whole invocation ownership model. The handoff path is complete now; attach-capable containment remains an allowed later optimization. |
| Keep forced drain, exempt all upgrade-adjacent failures from the counter, then run `lf runs reconcile` | Small diff, but the human session is still destroyed, recent zombies remain age-guarded, and broad temporal exemptions can hide genuine work failures. | It treats symptoms after losing the only evidence needed for an exact handoff. |
| Compare only `lf --version` before promote | Meets the narrow happy path but silently accepts replaced release assets, mismatched daemons/apps, or a drifted migration frontier. | Full installed identity is already available and turns a plausible no-op into a proved no-op. |

## Key decisions

- A real Home upgrade is a durable transaction; an exact reinstall is not an
  upgrade and must create no generation or receipt.
- Non-resumable interaction is drained before activation and is never forcibly
  killed. Failure to obtain a clean handoff stops the upgrade.
- An infrastructure interruption is evidence about the Home, not evidence that
  Task work failed. It has its own typed Task event and Run trigger and never
  enters `retry_of`.
- Invocation cleanup follows exact process ownership. Timestamps and missing
  rows alone never authorize settlement.
- The promotion fence is an admission barrier as well as a Run-reservation
  barrier. Activation requires a stable inventory after the barrier closes.
- Handoff artifacts are private, minimal, and immutable. They explain how to
  restart without copying prompt bodies or provider session material.
- The dangerous failure mode is a false liveness conclusion that interrupts an
  unrelated interactive process. Exact receipt matching, ancestor-cycle
  detection, pre-drain snapshotting, and fail-closed `unprovable` handling are
  release blockers, not polish.
- Do not change `studio`, build a generic host-upgrade platform, or replicate the
  mechanism to Cadenza in this Task. Loopflow must prove the shape first, matching
  the Wave bound against premature multi-product infrastructure.

## Scope

- In scope: installed-target identity and no-op promotion; Home-upgrade receipt
  schema and bridge persistence; Exec ancestry/liveness inventory; private
  handoff artifacts and log presentation; controlled TUI fence polling;
  transactional interactive admission fencing; stable pre-activation rescans;
  graceful non-resumable drain; typed upgrade-owned Run settlement; Task
  liveness/recovery classification; post-reconciliation invocation sweep; and
  focused plus staged behavioral proofs.
- Out of scope: edits to `studio/hosts/refresh-lf.sh`; automatic reconstruction
  of an unknown Claude/Codex TUI session token; a general unsupervised Run type;
  changing the three-attempt work-failure budget; keeping old `lf` processes
  alive across schema activation; Cadenza parity; and a generic multi-product
  deployment framework.

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
2. Upgrade with a trap-recording TUI fixture. The fixture observes SIGTERM, never
   SIGKILL; the log names a mode-`0600` handoff artifact; the invocation and Turn
   are terminal/interrupted; and the upgrade receipt records the same path and
   settlement.
3. Kill one Task body through each of two distinct staged upgrades. SQL over
   `task_events` shows two `home_upgrade_restarted` events and no corresponding
   `Failed { error: "task process is missing" }`; SQL over Runs shows no new
   `retry_of` link. A subsequent genuine body failure still receives recovery
   attempt one.
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
- handoff-required owners receiving SIGKILL: **0**;
- completed upgrades with captured dead-owner invocations still open: **0**;
- Home-upgrade missing-process observations emitted as Task `Failed`: **0**;
- change in a Task's work-failure recovery count across consecutive upgrades:
  **0**; and
- handoff-required invocations with either a private artifact or preserved live
  containment: **100%**.
