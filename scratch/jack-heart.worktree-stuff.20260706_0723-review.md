# Gate review: worktree identity and land flow

## What was implemented

Added `WaveId` as the source of truth for wave, subwave, and worker identity.
It separates the local worktree projection (`loopflow.bugs.fix.20260706_0801`)
from the remote branch projection (`jack/bugs.fix.20260706_0801`) so author
namespacing no longer leaks into filesystem paths.

`lf op wt create` now roots independent worktrees from main by default and uses
`--child` for explicit stacked descendants. `lf op land` / `submit` no longer
rotate or rename the live worktree; workers are expected to own PR lifecycles
without moving the persistent wave home.

## Key choices

- Keep one identity parser: `WaveId::parse` accepts branch or directory forms,
  then emits strict branch/path projections.
- Use `/` only in branch names for author scoping; keep worktree directories as
  flat `.` chains.
- Treat human-created children as persistent subwaves. Stamped worker identities
  are created by dispatch/runtime paths.
- Preserve namespaced upstream branches in worktree listing by stripping only
  the `origin/` prefix, not everything before the last slash.

## How it fits together

`engine/identity.rs` models the chain, author, and optional worker timestamp.
`engine/worktrees.rs` uses that model to plan and create worktree directories,
branches, worker IDs, list rows, and fresh stamped branches. Ops code calls
those helpers instead of reconstructing branch names from config schema strings.

## Risks and bottlenecks

- Branch names now contain `/`, so any remaining ad hoc branch parsing is the
  main regression risk. Gate found and fixed one instance in upstream parsing.
- The old `next`/`advance` endpoints still exist for compatibility with the
  current Concerto/wire surface. Stage 3b should retire them with the worker-mind
  runtime rather than another mechanical rename.
- `Concerto` UI tests did not complete locally: the app/unit portion passed, but
  the UI runner was killed before bootstrapping once, then a direct rerun failed
  to overwrite the previous DerivedData test bundle. This appears local runner
  state, not a branch behavior failure.

## What's not included

- Worker-mind runtime.
- Subwave dispatch flag.
- Retiring `lf op next`, `advance`, or `next_wave_handler`.
- Changing Swift DTO/wire contracts for the future worker runtime.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test -p loopflow --test worktree_tests worktree_list_preserves_namespaced_upstream_branch` passed.
- `cargo test -p loopflow --test land_tests submit_does_not_rotate_worktree` passed.
- `uv run python scripts/test.py` passed:
  - Rust: 1219 passed, 3 skipped.
  - Website: 61 passed, 3 skipped.
- `uv run python scripts/test.py --all`:
  - Python: 54 passed.
  - Rust: 1219 passed, 3 skipped.
  - Website: 61 passed, 3 skipped.
  - Swift: passed.
  - E2E smoke: passed.
  - Concerto: failed locally with `ConcertoUITests-Runner ... Early unexpected exit`; direct rerun failed at link time because Xcode could not write the prior UI test bundle in DerivedData.
