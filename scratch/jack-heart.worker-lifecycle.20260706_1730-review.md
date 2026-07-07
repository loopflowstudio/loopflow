# Worker lifecycle gate review

## What was implemented

This branch lands the S1 dispatch-targeting slice for worker lifecycle work.
Fresh dispatches now create a worker branch that forks from the owning wave
branch when that branch exists, and target that parent branch for review/landing.
Bare waves still fall back to the default branch.

Stacked dispatches keep their existing behavior, but now share the same
freshest-parent resolver: use the local parent branch when it has unpushed work,
use `origin/<parent>` when local is strictly behind, and fail when an explicit
stack parent exists nowhere.

## Key choices

- Keep workers on their own stamped branches instead of checking out the target
  branch directly. This preserves one worker branch per run while making the
  review target be the parent.
- Treat a missing fresh parent as a default-branch fallback. A bare wave has no
  branch yet during early setup, so dispatch can still work.
- Treat a missing stack parent as an error. Stacks are explicit dependencies; a
  missing parent there means the caller or registry is wrong.
- Remove the committed `.lf/scratch-stash` duplicate. The live design note in
  `scratch/worker.md` is the review artifact; the stash copy only duplicated it.

## How it fits together

`create_run_for_placement` still creates the `Run` row and records lineage.
`create_run_worktree` now computes both the branch start point and the
`Run.target_branch`, while `create_stacked_run_worktree` asks the shared parent
resolver for the correct local-or-remote parent tip.

The tests in `rust/loopflow/tests/wave_worktree_tests.rs` pin the two fresh
dispatch cases: bare waves fork from `main`, and waves with an existing
`<user>/<wave>` branch fork from that branch and target it.

## Risks and bottlenecks

- Existing callers that interpreted `target_branch` as "track this branch
  directly" now get "fork from this parent and target it." The CLI dispatch path
  already passes the current branch for this purpose.
- The fallback-to-default behavior is intentionally limited to fresh dispatch.
  If a wave branch was expected to exist but does not, the worker will still
  start from `main`; reviewers should validate that this is acceptable for S1.
- Background upstream sync still owns pushing new worker branches. The stack test
  avoids manually pushing the parent worker branch because that races the sync
  thread.

## What's not included

- Execs do not yet run inside an existing worker worktree.
- No minded terminating worker CLI exists yet.
- No cascade, sealing guard, prod verification oracle, or raw-session attach
  surface is implemented in this slice.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `uv run python scripts/test.py` passed the changed-aware Rust suite:
  1,239 tests passed, 3 skipped.
- `uv run python scripts/test.py --all` was attempted. Python, Rust, website,
  and Swift package tests passed before the Concerto Xcode UI job stopped
  producing output after building the UI runner. I interrupted it after several
  quiet minutes to avoid leaving a hung gate process running. Xcode wrote:
  `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-edortmcwhqfgbybchhwnmviqshkn/Logs/Test/Test-Concerto-2026.07.06_18-47-44--0700.xcresult`.
