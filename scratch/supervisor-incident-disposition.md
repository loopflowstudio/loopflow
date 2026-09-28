# Retained incident reports: remaining implementation

2026-09-28 · LOO-298 · Supervisor source inspection at `2e7571019`.
No production edits, provider mutations, or behavioral tests in this inspection.
This note narrows the remaining investigation; it does not supersede the design.

## Publication missing from the Task

The original report is Task comment `df090914-0a74-4507-b808-7348df2fb38b`:
PR #1296 was created without its Task PR row being populated.
Fresh installed `lf task status LOO-298 --json` now returns active row
`pr_64160469a52645eaafa913a5e024d2db` with publication GitHub number **1296**,
its URL and recorded head `8ccb0bde9ff84d843d4244add77986c62a59e89c`.
The missing association is no longer present on this Task. That observation
does not identify the original cause or prove all publication paths repaired.

Current source `ops/pr.rs::create_or_update_pr` requests durable publication
before GitHub mutation and calls `ops/task.rs::attach_task_github_pr` afterward.
`tests/task_pr_authority_tests.rs::valid_authority_publishes_and_records_the_pr`
asserts the stored publication and GitHub number with a simulated GitHub.
Keep this behavior during Exec/Session conversion; run its focused proof when
the conversion changes the Task authority path. No new parallel PR ledger.

## Stacking an existing Task: still missing

`ops/task.rs::prepare_task` accepts `stack_on` for an existing Task only when
the requested parent already matches its active PR's `parent_pr_id`. A Task
rooted on main is rejected; a different parent is rejected. `task_stack` is a
reader used by rebase, not an operation that changes the parent. The CLI exposes
`--stack-on` on prepare/run/create but no separate adoption operation.

The original requested behavior remains unimplemented. Complete it through the
existing placement/rebase/Task PR owners, preserving Task and worktree identity,
authored changes, PR chain and execution history. Prove an already-prepared Task
can acquire the selected parent, retry consistently and retain work on failure.
Do not claim that merely parsing `--stack-on` or rejecting an inaccessible store
proves this behavior.

## Cancellation and refused starts: existing replacement to retain

Original comment `bb5410ba-cc7a-4247-824e-4ec9c4f3fda6` requested cancellation
because duplicates LOO-299–302 remained in Linear, including issues created by
refused stacked starts. Later accepted LOO-305 decisions replaced that proposed
cancel/abandon surface with `lf task delete`; Infrastructure memory records the
decision and evidence. Do not recreate `pm task cancel` or `task abandon` from
the older report.

Current `ops/task.rs::task_delete` calls the existing provider/local deletion
operation. `task_create` passes `prepare_task_creation` into
`task_pm::create_and_load_task`; preparation resolves placement, Flow and agent
preflight before issue creation. Post-create allocation failures retain the
issue and name its retry, rather than hiding the external effect.
`ops/pm/task_planning_tests.rs` includes a refused-preparation/no-creation case;
`ops/task.rs::task_preparation_rejects_unpublished_parent_before_allocating_child`
covers the rejected stack parent. `tests/task_deletion_tests.rs` exercises the
real binary with synthetic Linear, on Linux only because of its fixture CA.

Disposition: preserve and verify the existing LOO-305 implementation where this
cutover changes its ancestry/Started/claim inputs. No duplicate cancellation
feature or fresh deletion of those historical issues is needed. Provider deletion
still does not establish process termination. These are inspected source/test
contracts, not newly executed acceptance results.
