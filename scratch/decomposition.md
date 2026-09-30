# LOO-298 decomposition

2026-09-30 · Jack Heart requested independent slices to land before the unfinished
model. Baseline `16258334e`, main `69b564d8e` (v0.12.26): 186 commits,
462 changed files, +54,616/−58,087 lines. Preserve branch
`jack-heart/data-model-one-table-per` and PR #1296 for LOO-334's stack.

## Landing order

| Slice | Existing commits / selected paths | Done when |
| --- | --- | --- |
| `resource-recovery`: bounded busy-cache cleanup | `7f520a016`: complete diff for `scripts/resource_envelope.py`, `python/tests/test_resource_envelope.py`, and its four-line TESTING.md paragraph | Independent main-based tree passes resource tests and Python lint; busy cache preserves its owner while build recovery continues; publish and land. |
| `pr-publication-continuity`: retain acknowledged Task PR identity | Publication hunks from `0d77e15cf`, `3c68ba8a7`, `38d690e23`, `a969f5807`: `ops/pr.rs` excluding Exec renames; `attach_task_github_pr` in `ops/task.rs`; publication-only additions in `pr_tests.rs`, `task_pr_authority_tests.rs`, `task_pr_range_tests.rs` | Main-based tree builds, affected PR suites and Rust formatting/Clippy pass; failed read/promotion retains identity and reviewer copy, retry creates no duplicate; publish and land. |

These are independent fixes, ordered for delivery convenience. Extract existing
hunks; do not import model types or add adapters to make either slice compile.
Each receives its own main-based worktree through `lf wt`, concise slice design,
and standalone proof. Use installed control tooling only for delivery, isolated
Homes and scrubbed inherited authority for source tests. No promotion or installed
Home conversion. `land -c` applies only to the extracted unbound slice, never
LOO-298's still-unfinished Task.

## Rejected seams

- Landing fixture `786d08d1a`: changes `op:` to `cmd:` in the Flow wrapper added
  by the Exec-per-step proof. Main directly calls `pr land` and has no wrapper
  to repair. Extracting it would add model-specific test setup, not fix main.
- Naming `6b2b9f2a6` / `16258334e`: 101-file rename includes model-only Session
  event stores, import readers, Flow inventory, native history and new DTOs.
  Session capture names describe the ownership change. Backporting a partial
  naming scheme would create churn and a different change, not an honest slice.
- Wave-service deletion and Chapter rotation: their migrations, finite execution,
  shared driver, installation and desktop replacements cross the same ownership
  cutover. Removing old owners ahead of their replacements would half-change main.
- Provider interruption, native recovery and metadata discovery use the new Exec
  and Session owners in their behavioral proofs. Keep their replacement together.
- Model docs/generated pages and historical scratch records describe the new
  contract; they do not become independent documentation PRs.

## Remainder on #1296

Exec / AgentSession / FlowSession ownership, migration and historical preservation,
Exec-per-step and common command behavior, typed decisions, attribution, native
recovery, Chapter model, service removal, naming and coordinated Desktop/wire
changes remain one architectural change. Keep released-populated import, final
docs/skills/pages, configured acceptance and Jack's demo open. The `agent_events`
and `exec_events` table renames remain proposals for Jack.

After both slices merge, rebase #1296 through Loopflow and publish the remainder;
do not land, close, rename or complete it. Preserve the original branch history
and all scratch evidence throughout extraction. Record standalone results and
authoritative merge receipts below; fixture success is not configured acceptance.

## Standalone verification and review

- `resource-recovery` / [#1358](https://github.com/loopflowstudio/loopflow/pull/1358):
  all 12 resource tests pass; Ruff and diff checks pass. Original implementation
  and test hunks were applied unchanged. Hosted PR and merge-queue checks passed.
  `lf pr land -c` confirmed merge `1f5f0c4e45c79ca45cd1b6f4c0cdf7427ae9f12e`.
- `pr-publication-continuity` / [#1359](https://github.com/loopflowstudio/loopflow/pull/1359):
  all 50 runnable PR/Task-authority/Task-range cases pass (one skipped), no
  fail-fast. Formatting, all-target Clippy with warnings denied, and diff checks
  pass. Logs: `.lf/tmp/decomposition-pr-tests.log` and
  `.lf/tmp/decomposition-pr-clippy.log`. The resource preflight passed above the
  32 GiB reserve. Hosted PR and merge-queue checks passed. GitHub confirms merge
  `00cf9dffe840d0c96c579667b8da22dfa03939bc` at 2026-09-30 17:00:33 UTC.
  The `lf pr land -c` watcher returned "no auto-merge request" at this boundary;
  a direct authoritative read showed MERGED, with no queue entry or merge request.
  No duplicate merge or repair operation followed that counterexample.

Review kept acknowledged identity persistence and later Linear linkage together;
an incomplete hunk extraction would have removed writeback. Inspection corrected
that extraction before building. The slice retains promotion-failure copy,
head-pinned merge invalidation and retry behavior without schema or model changes.
Both slice PRs target main and carry no LOO-298 Task binding. Installed control
commands clear inherited Task/Run authority for these independent checkouts;
source proofs use private Homes. No branch binary accesses the installed Home.

## Remainder integration

`lf rebase --manual` rebased the original branch onto main `00cf9dffe` after
both merges. Two replay stops touched publication code and its tests; retaining
the landed final publication ordering resolved them. The immediate comparison of
all 2,040 non-scratch tracked files against the pre-rebase snapshot has **zero
differences** (`.lf/tmp/decomposition-rebase-comparison.json`). The 188-commit
branch history and its name remain intact; LOO-334 was not changed.

The four publication failure/retry tests pass on the rebased model branch
(`.lf/tmp/cut-i/decomposition-rebase-focused.log`), with simulated GitHub and
private stores. Formatting and diff checks pass. This is the focused conflict
proof, not a full local matrix or final gate. Remaining import, documentation,
configured acceptance, Jack's demo and both proposed event-table names stay open.

Final all-target compilation found an existing v0.12.26 integration gap in the
model branch's two provider-account fixtures: missing observed identity/plan
fields. Adding those fields exposed the Codex fixture's missing identity. It now
supplies an expected email and matching synthetic ID-token claims, following
main's account contract. These are the only non-scratch differences after the
byte-identical rebase; production and migration files remain unchanged.

The first fixture run retained three Codex failures and one passing Claude
failure/retry/review case (`decomposition-account-fixtures.log`). All three
Codex repairs pass in `decomposition-account-fixtures-final.log`, preserving
Task Started, history, context and independent managed-Flow assertions. Final
all-target Clippy with warnings denied passes (`decomposition-final-clippy.log`),
as do formatting and diff checks. Logs live under `.lf/tmp/cut-i/`.
