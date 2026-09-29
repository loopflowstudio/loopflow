# Execution-model handoff

LOO-298 · Jack Heart · 2026-09-29.

The complete contract remains in [the design](data-model-one-table-per.md),
[remaining work](remaining-work.md), [import preservation](import-preservation.md)
and [Chapters](chapters.md). [Concept review](concept-review.md) identifies the
next ownership changes. No code-complete acceptance, Task completion or shipment
has occurred.

## Current order

Jack directed autonomous progress to code-complete, with a published checkpoint
per coherent slice:

1. Repair hosted Rust and run all tests left unrun by fail-fast before publishing.
2. Structured Flow results constrained by each captured boundary's schema;
   remove the in-turn decision/router commands and authority paths.
3. Captured input as a Session history event; remove RunId, the input catalog and
   SessionRecord.run_id while retaining historical evidence and launch fences.
4. Rebase the released history proposal onto event identity, include orphan native
   receipts with unknown attribution, then remove unnecessary wrappers.
5. Saved-Flow discovery with Desktop paging and reconciliation in the same cut.
6. Released-populated public import bridge and exact materialized-copy counterpart.
7. Reconcile docs, skills and generated HTML with final behavior.

The complete matrix remains binding; this order does not drop configured-provider,
Desktop, Chapter, incident or migration obligations. Final concept review with
Jack and resolved findings precede final gates and saved delivery. Usage after
bind remains prospective as an operating assumption pending Jack's decision.

## Current evidence

HEAD `537e7924d`, PR #1296. Hosted run `36619025873`: Swift and UI pass;
Rust 1,935 passed / one failed / 15 skipped / 64 unrun. The publication test now
passes. `wave_resolution_matrix::registry_is_complete` lacks `exec list`, whose
Wave argument filters recorded work. `exec show` has no Wave argument. The isolated no-fail-fast run (`ci-tail.log`) passed 72/72: all 64 hosted-unrun
cases, the repaired classification, and seven macOS-only cases. The failed
classification was first reproduced unchanged (`wave-matrix-red.log`). No green
hosted matrix is claimed before publication.

Runtime loop children already use the shared driver/settlement transaction:
retry retains the child, successful return occurs once, later passes are siblings,
and the managed Task pointer stays on the root. Do not rebuild this or early Exec
observation from stale checklists. Reuse unchanged proofs:

- `runtime-compress-final.log`: 29 focused passes.
- `runtime-children-canonical-final.log`: 22 ownership/Chapter passes.
- `review-runtime-repairs.log`: 16 runtime/publication repair passes, including
  public taskless looping. Providers/GitHub are scripted and remotes local.
- `chapter-public-cli-3.log`: public Project-default Task launch, rotation and
  second-private-Home sync adoption preserve identity, exact Started, worktree,
  PR and capture. Scripted Linux providers, not configured-provider acceptance.
- Hosted Swift at `067ff0164`, run `36614840195`, confirmed the off-window terminal
  attachment repair; the latest hosted Swift pass confirms it again.

Detailed earlier implementation, failures, commands and proof limits are retained
in [the committed handoff at 537e7924d](https://github.com/loopflowstudio/loopflow/blob/537e7924dee15005c94addafb56aad0bdd8bf48f/scratch/parallel-execution.md),
[evidence](evidence.md) and [runtime children](runtime-flow-children.md). The links
replace chronology here; they do not upgrade any recorded result.

## Working boundaries

Main owns this worktree's source, builds, integration and Git. Preserve supervisor
edits to questions, findings and the contributor index. Released proposals are
hunks to review, not whole files to overwrite:

- `.lf/tmp/final-history-proposal/`: uncompiled; retains RunId and omits orphan
  native receipts from aggregate discovery. Integrate only after event identity.
- `.lf/tmp/flow-discovery-proposal/`: uncompiled; review insertion-time Git lookup
  and command/template collisions, now using real runtime parentage.
- `.lf/tmp/released-import-proposal/`: authored, unexecuted public import bridge;
  require an executed result and effective provider isolation, source and canonical.

Use `uv run python .lf/tmp/cut-i/control-checkpoint.py ...` for installed control
operations only. Bare lf can choose another store. Never point branch bytes at
that installed data. Source proofs use disposable Homes with LF_/LOOPFLOW_ authority
removed by `.lf/tmp/cut-i/run.py`. No installation promotion is authorized.
