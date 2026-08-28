# Gate review: Project child-resume authority

## What was implemented

Project controllers now issue one opaque child-resume capability before
provider work begins. SQLite stores only its hash plus the exact controller Run,
Project phase, and Steer frontier. The ordinary `lf task resume` path validates
that capability before any Task or PR mutation and again before child launch.

Phase changes rebind the capability before the next provider step. Controller
recovery replaces both the holding Run and token, while retries reuse the
existing Task Work. Child execution and shell boundaries scrub the Project
capability.

## Key choices

- Execution Runs remain evidence and provenance, not implicit mutation
  authority. The Project controller owns capability issuance, advancement,
  replacement, and release.
- Main removed generic `work_flow_positions`; the rebased implementation keeps
  `flow`, `step`, `step_index`, and `iteration` in the scoped capability row
  instead of reviving deleted shared state.
- Resume retains its two intentional checks: the first protects PR
  reconciliation and other entry mutations; the second protects the later
  launch boundary from authority that became stale meanwhile.
- Direct User resume remains valid. Any in-Run caller must present the exact
  immediate Project capability.

## How it fits together

Project controller publication creates the Run capture and capability together,
then passes the raw token only to the Project provider environment. `lf task
resume` resolves the Task's immediate Project, hashes the presented token, and
matches the controller Run, phase fields, Ready Work, and current Steer
sequence. Child launch receives no Project token.

## Risks and bottlenecks

- The capability crosses one environment boundary, so every child and shell
  launch path must keep using the shared execution-identity scrub list.
- The draft migration must materialize into release tests; the fixture resolves
  it by marker rather than a compile-time draft path.
- The exact-head affected-suite gate selected full Python and release-materialized
  Rust. Both local attempts stopped before product tests because three other
  active worktrees exceeded the repository's 12 GiB per-build envelope.
  Recovery correctly removed nothing because those owners were active. The
  final flow must require exact-head CI evidence; this is host-pressure evidence,
  not a product failure.

## What's not included

This capability covers resuming an existing parked Task from its immediate
Project. Shared Wave overrides such as `task steer` and `task run`, Project
controls below a Wave, and User-only terminal recovery need a separate
Wave-owned authority design before they can be tightened coherently.

## Validation

- Post-rebase behavior passed:
  `cargo test -p loopflow project_runner_control_resumes_task_across_phase_and_process_recovery -- --nocapture`.
- `git diff --check origin/main...HEAD` passed.
- The checked architecture map now owns `project_child_controls` explicitly:
  `uv run python scripts/check_architecture.py` reports 35/35 SQLite owners,
  and `uv run pytest python/tests/test_architecture.py -q` passes 19 tests.
- Negative source review found no runtime `agent_turns`,
  `validate_control_caller`, or `work_flow_positions` authority reader. Those
  names remain only in migration reconstruction and historical fixtures.
- The changed-aware plan selected Python and Rust, skipped slow E2E for CI, and
  recorded the resource-preflight stop before running product suites.
- Exact-head CI independently passed release-materialized Rust tests, Rust
  formatting/clippy, Swift tests, the Loopflow UI compile check, E2E smoke,
  migration validation, and website tests. Its Python job passed 204/205 tests
  and exposed only the missing architecture-map entry fixed by the focused
  proofs above. `scratch-clear` is expected to remain red until the final flow
  consumes this review handoff and clears the Task scratch files.

This advances Infrastructure's state-preservation and least-authority goals:
the reported parked Task remains singular across retry and recovery, stale
controllers fail before mutation, and child processes receive no Project
authority.
