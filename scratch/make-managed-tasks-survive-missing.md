# Make managed Tasks survive missing lifecycle flows

## Problem

A managed Task pins the three flows that define its lifecycle. If one of those
flows later disappears, launch validation rejects the Task, but Project
supervision historically treated the refusal as transient: each tick reached
the same recovery path again. The result was repeated recovery attempts rather
than one durable, actionable non-convergence boundary.

LOO-167, LOO-193, and LOO-195 exposed the failure with the retired `task` loop
flow. The exact historical default has since been repaired to `slice` by the
released `repair_legacy_task_flow` migration. Current lifecycle selection is
otherwise immutable: an unknown invalid pin must be restored, or the Task must
be abandoned and replaced. Runtime code must not guess a replacement or add a
compatibility alias.

## Integrated baseline

The current base already provides the rest of the LOO-207 contract:

- Task creation expands and structurally validates all three flows before
  persisting Task or PR state.
- Every Run launch revalidates the persisted lifecycle before reservation.
- Status projects an invalid persisted lifecycle as `no_action` with the exact
  refusal.
- Missing and initializing worktrees remain readable and preserve Task identity
  and PR history.
- The released `task → slice` migration changes only the retired historical
  default; tests cover the recorded LOO-167, LOO-193, and LOO-195 identifiers.

This branch closes the remaining resident-convergence gap.

## Approach

Before either resident path reconciles PRs or relaunches a body:

1. Validate the persisted lifecycle through the same generic validation used by
   Task launch.
2. If validation succeeds, continue through ordinary recovery.
3. If it fails, append one resumable `Failed` Task event containing the exact
   validation error and stop that tick.
4. On later ticks, compare the latest event with that exact error. If it already
   matches, park silently without another event, warning, PR mutation, or Run.
5. If the pinned flow is restored, generic validation succeeds and ordinary
   recovery becomes legal again.

The check runs both in Project-wide supervision and after a lost Run is
recovered, so alternate liveness entrypoints share the same boundary.

## Key decisions

- Persist failure evidence on the Task. Logs alone cannot settle a durable
  recovery loop.
- Keep the event resumable. The Task is blocked by its current configuration,
  not terminal; restoring the exact flow makes the same Work executable.
- Compare the full error. A changed validation result is new evidence and gets
  one new receipt.
- Warn only when the receipt is first written. Repeated supervision ticks are
  expected and should be quiet.
- Do not revive the removed `task` flow or mutate an arbitrary pin at runtime.
  The released historical data migration and immutable current selections are
  separate boundaries.

## Scope

In scope: resident lifecycle validation, one durable refusal receipt, and proof
that repeated ticks create no Runs or PRs.

Out of scope: another legacy migration, compatibility aliases, arbitrary flow
renames, worktree reconstruction, and changes to Project or Wave lifecycles.

## Done when

- Two Project supervision ticks over an invalid persisted lifecycle produce one
  resumable failure event containing the missing flow name.
- The same proof observes zero Runs and an unchanged single-PR sequence.
- Lost-body recovery reaches the same parking boundary.
- The existing historical migration proof for LOO-167, LOO-193, and LOO-195
  remains green.
- `cargo test -p loopflow --lib resident_records_an_invalid_lifecycle_once_without_retrying`
  passes.
- `cargo test -p loopflow --lib task_lifecycle` passes.
- `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings` pass.

## Measure

Before: every resident tick could attempt the same invalid recovery again.

After: the first tick writes one actionable failure receipt; later ticks write
zero events, reserve zero Runs, create zero PRs, and emit no repeated warning.
