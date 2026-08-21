# Review: make managed Tasks survive missing lifecycle flows

## What was implemented

Resident Task recovery now validates a Task's persisted lifecycle before PR
reconciliation or relaunch. An invalid lifecycle writes one resumable `Failed`
event with the exact validation error, then parks. Later ticks that see the same
latest receipt make no store change and emit no repeated warning.

The boundary is shared by Project-wide supervision and lost-body recovery. The
focused proofs observe one failure event, zero Runs, and an unchanged PR chain.

## Key choices

- Reuse `validate_task_lifecycle`, the same expansion and structural validation
  used before every Task Run reservation.
- Persist the refusal as Task evidence rather than treating a log line as
  convergence.
- Keep the failure resumable so restoring the exact pinned flow makes ordinary
  recovery legal again.
- Compare the complete error string. A changed lifecycle failure is distinct
  evidence and receives one new receipt.
- Warn only when writing the receipt; expected later supervision ticks are
  silent.
- Keep lifecycle selection immutable. The historical `task → slice` repair
  already shipped in `0.12.5.001_release.sql`; this branch adds no alias or
  runtime rewrite.

## How it fits together

Project supervision and missing-containment recovery both pass through
`park_invalid_lifecycle`. Valid Tasks continue into the existing adoption, PR,
and relaunch paths. Invalid Tasks stop at a durable event that status already
projects as `no_action`, so runtime behavior and the user-facing next owner
agree.

## Risks and bottlenecks

- Idempotence compares the latest Task event before appending. This relies on
  the existing single active Project supervisor per Work; it is not a database
  uniqueness constraint across concurrent writers.
- Validation loads and expands all three pinned flows on each active Project
  tick. That is intentionally the same proof required before launch; this branch
  adds no network call or provider startup to the tick.
- A repo-local flow cannot be validated while its worktree is absent. Existing
  status/action projection gives the worktree blocker precedence, preserving
  the exact path and branch recovery instruction.

## What's not included

- No compatibility `task` flow.
- No second legacy migration or arbitrary flow-name rewrite.
- No automatic worktree reconstruction.
- No Project or Wave lifecycle changes.
- No live mutation of the recorded LOO-167, LOO-193, or LOO-195 Tasks.

## Validation

| Proof | Result |
|---|---|
| `cargo test -p loopflow --lib resident_records_an_invalid_lifecycle_once_without_retrying` | 1 passed |
| `cargo test -p loopflow --lib lost_body_parks_an_invalid_lifecycle_without_relaunching` | 1 passed |
| `cargo test -p loopflow --lib repaired_persisted_task_flows_validate_through_the_generic_path` | 1 passed; LOO-167/193/195 fixtures |
| `cargo test -p loopflow --lib task_lifecycle` | 7 passed |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --all-targets -- -D warnings` | passed |

The full Rust matrix remains CI-owned per `TESTING.md`; no UI or external
provider path changed.

## Wave alignment

This advances the product Wave's Task-loop trust KR: a missing lifecycle now
settles as one actionable non-convergence receipt instead of repeated automatic
recovery. It also preserves the shared API model by reusing the launch validator
that status and Task execution already consume.
