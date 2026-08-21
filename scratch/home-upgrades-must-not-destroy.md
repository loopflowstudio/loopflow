# Home upgrades leave live work alone

## Problem

On 2026-08-19 the Home repeatedly promoted the already-installed 0.12.8
release. Every promotion created a new runtime generation, paused the Home, and
drained all Runs. Interactive sessions were killed, one Invocation remained
`running/capturing` after its process disappeared, and Task supervision counted
upgrade-caused process loss against the three-attempt work-failure budget.

Four boundaries are missing:

- an exact reinstall is not an upgrade;
- a promotion may stop only work Loopflow knows how to resume;
- a Home interruption is not a Task failure; and
- a dead process cannot leave an Invocation recorded as running.

The existing promotion lock, `home_upgrades` / `home_upgrade_work` receipts,
typed `HomeUpgrade` stop cause and Run trigger, capture artifacts, and exact Exec
process receipts are enough to establish those boundaries. Do not add a second
invocation inventory or handoff protocol.

## User-visible contract

```text
$ studio/hosts/refresh-lf.sh
0.12.x already installed; fleet untouched
```

The generation, Run IDs, and process IDs do not change.

For a real upgrade, Loopflow either proves every affected body auto-resumable
or leaves the fleet untouched:

```text
promotion deferred: live non-resumable Invocation invocation_…
  events: ~/.lf/traces/…/conversation.jsonl
```

The session remains live. Finishing it and rerunning the scheduled refresh is
the handoff; promotion never kills it to make progress.

Auto-resumable Task bodies quiesce and relaunch with `HomeUpgrade` lineage.
They do not emit a Task failure, enter a `retry_of` chain, or spend recovery
budget. If a process nevertheless disappears, the completion sweep settles its
Invocation as interrupted and prints the already-preserved conversation
artifact instead of inventing another artifact format.

## Design

### 1. Return before an exact reinstall becomes an upgrade

Under the exclusive promotion lock, after read-only preflight and before
creating an upgrade receipt or staging artifacts, compare the requested
installed targets with the candidate:

- the installed CLI's preflight identity equals `CandidateIdentity`;
- the installed daemon reports that same identity; and
- when app installation was requested, its bundled helper reports that same
  identity and verdict.

Missing or mismatched targets take the real-upgrade path. A complete match
prints `already installed; fleet untouched` and returns without creating a
`home_upgrades` row, incrementing `home_runtime_generations`, pausing the Home,
or reconciling Work. Explicit skill synchronization may still run because it
does not replace the control plane.

Compare full published identity, not only display version. Equal version text
with different source or migration identity is drift, not proof of a no-op.

### 2. Stop only bodies already proven auto-resumable

Before pausing the keeper or sending a stop request, make one store decision:

1. Read every open Invocation on this Home.
2. Classify as auto-resumable only a headless Invocation supervised by a Run
   whose Work the existing upgrade reconciler can relaunch.
3. Treat every other live or unprovable owner as a blocker.
4. Either return the blockers without changing Run state, or atomically mark
   the captured Runs `stopping` with `StopCause::HomeUpgrade`.

This is capability-based and fail-closed. `tui`, `ide`, `ask_tui`, unsupervised,
and unknown future surfaces are blockers unless they later gain an explicit
resume contract. No surface list asserts that a body is disposable.

Invocation creation checks the active promotion fence in the same transaction
that inserts the capture. Re-scan immediately before forced signalling so the
first upgrade from an older binary also fails closed if it admitted a capture
without that check. A blocker rolls the planned upgrade back before Home pause
or any signal.

This deliberately defers all real upgrades around non-resumable interaction.
Keeping an old interactive process alive across a new control-plane generation
and distinguishing migration-compatible writers would be a larger protocol
without improving the core safety guarantee.

### 3. Keep Home-upgrade causality out of Task recovery

Replace `finish_absent_run`'s installer-local SQL with one store transaction for
settling an upgrade-stopped Run. It:

- preserves `StopCause::HomeUpgrade { upgrade_id, … }`;
- ends open Turns and Invocations as `interrupted`;
- ends the Run; and
- emits no Task or Project work-failure event.

Task and Project liveness reconciliation read the typed stop cause. When the
cause names the active upgrade that captured the Run, they leave settlement to
the upgrade coordinator instead of calling ordinary `recover_run`.

The existing upgrade reconciler relaunches enabled Work with
`RunTrigger::HomeUpgrade`; `reserve_run_in` already leaves `retry_of` empty for
that trigger. The exemption is tied to the matching active upgrade, not merely
to a Run that once started after an upgrade. A later unrelated process loss is
an ordinary failure and starts at recovery attempt one.

### 4. Reconcile exact dead owners before completion

Extract one exact-owner sweep from the current capture reconciliation path. Run
it after the exact-reinstall decision, before a real upgrade drains Runs, and
again after Work reconciliation, before the upgrade becomes `Completed`. The
no-op path remains wholly read-only.

For each open `capturing` Invocation:

- follow `run_events.parent_process_id` within its trace to an
  `ExecProcessReceipt`;
- consider the owner dead only when the receipt's PID, process start time,
  trace ID, and Exec ID match and that exact process is absent;
- atomically end running Turns, set the Invocation to
  `capture_status=interrupted`, `outcome=interrupted`, and record the upgrade in
  `incomplete_reason`;
- preserve its existing conversation and provider-session artifacts; and
- remove the exact stale Exec receipt only after settlement commits.

Already-terminal Invocations are unchanged. Missing ownership evidence remains
unprovable and blocks destructive work; age and PID alone never authorize
settlement. Repeating the sweep is a no-op.

This detects the `invocation_74115449…` shape without a
`home_upgrade_invocations` table: its trace ancestry reaches an exact stale Exec
receipt, and its conversation capture is already the handoff artifact.

## Scope

In scope:

- exact installed-target no-op detection;
- fail-closed resumability classification at the promotion boundary;
- transactional Home-upgrade Run settlement;
- Task/Project liveness respecting the typed upgrade cause; and
- exact dead-Invocation reconciliation at upgrade start and completion.

Out of scope:

- preserving old interactive writers across a new runtime generation;
- a new handoff file, invocation inventory table, or generic resumability
  registry;
- edits to `studio/hosts/refresh-lf.sh`;
- general status rendering, owned by the adjacent stale-status task;
- changing the three-attempt work-failure budget; and
- Cadenza or a generic deployment platform.

## Proof

Use focused Rust tests plus one staged upgrade fixture:

```bash
cargo test -p loopflow lf::commands::install
cargo test -p loopflow ops::task
bash tests/e2e/test_home_upgrade_survival.sh
```

The fixture proves:

1. Promoting the same complete candidate twice logs `fleet untouched`; the
   second call creates no upgrade or generation and preserves exact Run and
   process IDs.
2. A real promotion with a live TUI fixture defers before Home pause or signal,
   prints the Invocation and conversation artifact, and leaves the process and
   Invocation live. After natural exit, the next promotion proceeds.
3. Two distinct upgrades quiesce and relaunch one headless Task body without a
   `TaskEventKind::Failed` event or `retry_of` link. A subsequent genuine body
   loss consumes recovery attempt one.
4. An unsupervised nested capture with the
   `invocation_74115449…` stale-owner shape settles as interrupted, preserves
   its conversation artifact, removes only its exact stale receipt, and stays
   unchanged on a second sweep.

The essential invariants are visible directly in `home_upgrades`,
`home_runtime_generations`, `runs`, `agent_invocations`, and `task_events`; no
new telemetry schema is required.
