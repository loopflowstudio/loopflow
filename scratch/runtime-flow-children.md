# Runtime loop ownership

**Superseded by Jack Heart's 2026-09-30 decision.** One started Flow is one
FlowSession; passes are node/iteration positions and history lenses. Item 2 in
[remaining work](remaining-work.md) removes child-pass storage with a preserving
forward migration. The implementation and review below remain historical evidence.
Keep their behavioral proofs, rewrite assertions around positions, and delete
tests whose only claim is that child rows exist.

LOO-298 · Implementation boundary, 2026-09-29.

Jack's accepted contract requires durable children for runtime loop passes. The
captured graph has backward edges, including overlapping ranges, rather than
static loop blocks. Taking an Iterate edge enters a child pass; initial forward
execution remains in its existing FlowSession. Template expansion and XOR routing
alone create no FlowSession. This interpretation preserves the authored graph.

The waiting parent retains its deciding boundary. A child has the same captured
graph and nullable Task, starts at the selected backward target, and returns at
that decision. Inner backward edges create children; Iterate at the returning
decision creates the next sibling pass. Successful settlement, parent continuation
and child allocation share the existing version/claim transaction. Failure and
interruption retain child identity. The managed Task pointer continues to name
the root; execution reads descend to its current child.

Proof: Task and taskless drivers, failed/interrupted child recovery, exactly one
parent continuation, another pass with a distinct child, stale writes rejected,
overlapping edges and XOR preserved. A metadata-only parent column does not count.

Implementation and proofs now live in the existing
[main handoff](parallel-execution.md#runtime-children-and-ci-repairs--implementation-iteration-11).
The complete remaining matrix, import and Chapter obligations remain unchanged.

## Slice review — 2026-09-29

Scope: `c928d261c` through `d18cac7d2`, plus the bounded publication repair below.
The reviewed path is the shared Task/taskless driver, checkpoint transaction,
managed membership readers and retained-terminal repair. Jack's complete-model
concept review and the remaining import/history/discovery work are still owed.

| Claim | Evidence and result |
| --- | --- |
| Runtime children execute and return once | Fresh `review-runtime-repairs.log` public CLI case passes: two sibling passes, three completed Flow rows, six consumed successful turns, no Run table, and completed-root resume launches nothing. Scripted OpenCode in a disposable Home; no configured provider. |
| Failure/interruption retains the child; stale success cannot settle | Source inspection confirms the root lock and existing version/claim/selected-completion transaction. Reused unchanged `runtime-compress-final.log` has 29 passes, including reopened-store retry, stale settlement, overlapping returns, Task driver and child review. |
| Template/XOR expansion does not mint children; Task keeps one root | Reused Task-driver assertions compare every Session's membership before/after Iterate; child review checks the exact root pointer, released claim and rejected late completion. SQL has one active child per parent and validates immutable Work ancestry. |
| Upgrade preserves prior identity without inventing parents | Reused source populated-frontier pass and 22 materialized checks in `runtime-children-canonical-final.log`. This is not the still-pending released public import bridge. |
| Speculative mounts cannot detach the displayed terminal | Unchanged six local integrated cases and supervisor-reported hosted swift-test success at `067ff0164`, run `36614840195`. Hosted confirmation supersedes the earlier outstanding confirmation; no fresh Swift suite was run here. |
| Publication adopts an already-open PR without prior local publication | Hosted failure reproduced alone in `review-runtime-publication-red.log`. `attach_task_github_pr` required a local request before retaining observed GitHub identity. It now initializes only the missing publication envelope, preserving absent presentation and merge intent. |
| Adoption retains identity through failed promotion and retry | New regression and existing ready-PR/stale-base, draft, serial publication and refusal tests pass: `review-runtime-repairs.log`, 16 tests total in 15.403s. Git uses local bare remotes; GitHub and providers are simulated. |

All logs and the count receipt live under `.lf/tmp/cut-i/`. Formatting, all-target
Clippy (`review-runtime-clippy.log`) and diff checks pass. No full matrix reran.
The original hosted Rust result remains 1,917 passed, one failed, 15 skipped,
80 unrun; focused repair is not a new green hosted result.

Negative architectural proof: repeated work no longer executes solely beneath
the root's mutable cursor. The existing FlowSession owner now records the pass;
no new lifecycle table, driver or Task pointer was introduced. Runtime parentage
does not replace expanded template/XOR cursor structure. Review removes the
publication precondition that rejected valid existing GitHub state; it adds no
alternate publication writer or compatibility path.

Production measurement against `c928d261c`: **+437/−175, net +262** (Rust/Swift
+409/−175; SQL +28). Review repair alone against `d18cac7d2`: **+8/−6, net +2**.
`review-runtime-counts.json` uses the same production-prefix exclusions for tests,
docs, scratch and generated files. This adds required behavior, not a reduction
claim. The bounded review finding is fixed; the applicable runtime-child slice
criteria hold with the evidence limits above.

Next implementation: apply Jack's captured-event and structured-result decisions,
reconcile the released history/discovery/import proposals, and execute their
preserved public/canonical proofs. Keep `remaining-work.md`, `import-preservation.md`
and `chapters.md` as the full acceptance owners. The old legitimate Codex retry
failure remains until the chosen structured result path proves replacement.
No whole-design completion, Jack acceptance, Flow edge or merge follows here.
