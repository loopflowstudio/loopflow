# Continuation branch comparison — 2026-09-25

Human asked whether rebasing onto
`jack-heart/restore-task-continuation-with-a` would help. Parent recommends it
after the active bounded implementation pass, before the next compress/review.
No rebase has been applied. This question alone is not recorded as approval to
integrate, publish, or mutate the sibling checkout.

Subsequent execution decision: after the human confirmed both loops, the delivery
tail and continuing the build, the parent integrates the named runtime dependency
locally as necessary implementation work. No push, remote branch change, sibling
mutation or installed-runtime change is included. The bounded implement Run has
exited; compress/review follow integration. Preserve the whole coherent Task tree
in a local checkpoint before the reversible rebase.

Read-only `lf rebase --plan jack-heart/restore-task-continuation-with-a` reports
`direct_rebase`, protected current branch, 46 unique commits and 757 changed
files. This is a strategy preview, not a prediction of conflict-free integration.

Observed sibling source and scratch:

- `durable.rs::FlowPosition` replaces flat step_index/iteration/progress with
  the shared `ExecutionCursor`; leaf/path identity is material for nested XOR.
- `engine/execution.rs` and `engine/transitions.rs` own ordinary and Task
  traversal, backward edges and human revision. Counts do not impose pass limits.
- `ops/flow_run.rs` owns standalone invocation/boundary persistence and exact
  StepToken; `ops/flow_session.rs` projects its human boundaries through the
  existing Session kind and launch machinery.
- `scratch/cursor-integration.md` records isolated recovery/traversal proof.
  `scratch/live-loopflow/final-review.md` records accepted bounded live transport
  proof. These are sibling receipts, not freshly rerun proof of this checkout.

This directly overlaps cycle 2's new Flow membership and Session naming/open
paths. Reconcile those with exact cursor/standalone boundary authority rather
than extending the old flat managed-Task model or adding adapters. Keep names
and human provenance through both managed and standalone boundary recovery.

Resolved composition question: current sibling `task/flow/pursue.yaml` contains
both `decide -> implement` and a `decide_delivery -> implement` after human demo.
The human explicitly confirmed “The 2nd loop is correct.” This supersedes the
earlier single-loop topology in the prototype. The governing design and desktop
direction now accept both edges; source also uses `design` as the human design
skill. Render actual pinned definitions, preserving separate occurrences of
loop-decide and their per-edge traversal evidence. The original prototype source
and hash receipt remain unchanged historical visual reference.

Human follow-up: “we'll have to revisit the UI.” Revisit the two-loop diagram
visually before implementing the native Flow diagram. This does not cancel the
current named-Session slice or invalidate the accepted Session hierarchy.
The subsequent instruction keeps building until the working build is ready for
that discussion. Final Advance → queue → land is now explicitly confirmed;
final Iterate returns to implement. Delivery operations must use the existing
PR authority; no live queue/landing action is authorized by this design change.

The active implement subagent continues its already bounded pass. It must exit
before any checkpoint/rebase. Parent will inspect its receipt and preserve all
shared work before integrating if directed. No sibling dirty work was copied.
