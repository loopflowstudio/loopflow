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
