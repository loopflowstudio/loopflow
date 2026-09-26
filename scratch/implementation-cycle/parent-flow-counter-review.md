# Flow counter reconciliation required before review acceptance

Parent source observation during cycle 4: TaskFlowView.loopIteration derives
`edge.traversals + 1`, and the test calls this each loop's own iteration.
ExecutionCursor.iteration increments on either return at that cursor level.
With two returns via decide and one via decide_delivery, the same ongoing pass
cannot truthfully be labeled iteration 3 on one span and iteration 2 on the
other. Independent traversal evidence remains useful, but it is a different fact.

Preserve both authored edges. Before accepting the implementation, project the
actual saved cursor iteration (and nested level when applicable) through Rust,
then label execution iteration from that evidence; label per-edge counts as
returns/traversals. A product decision about one or two blue regions is deferred
and does not excuse incorrect labels. No second iteration counter/store or
Swift reconstruction from skill names. Reconcile the concrete test across both
return edges, including a return after demo. The concurrent Flow identity review
also records nested Session-to-node ambiguity; inspect the final source for it.

This is a review of an unfinished draft, not a completed slice verdict. Parent
will settle it after the child finishes its owned edits, before final acceptance.

## Parent correction — verified locally

Rust now projects each return span's optional, zero-based saved cursor iteration
at its actual nesting level. Swift consumes that evidence and presents one-based
iteration labels; per-arrow traversal counts remain separate and appear in help.
The two-edge fixture has different arrow counts but the same current iteration.
The real CLI regression now checks the iteration as well as both counts.

Session membership replaces the lossy public leaf index with the structural
node path from ExecutionCursor. New capture manifests record it; older manifests
retain known Flow provenance with an explicitly unavailable node, never infer a
path from their former leaf index. Graph and cursor use the same node-key builder.
A nested Session regression compares both direct boundary and captured Run
membership against the actual graph node. Session iteration labels use the same
one-based presentation. Required Rust/Swift mirrors and fixtures move together.

The configured Session test fixture still advertised removed approve/iterate
actions. It now exposes Complete through the same response path as Ask. This
fixture edit alone does not establish hosted UI interaction. The earlier note's
claim that the fixture Task is a visible planning row is not true of its empty
roadmap: it remains an unmatched Session. Do not change the test's navigation
just on that assumption.

Earlier cycle-4 receipts/captures describe the prior candidate. They must not
be relabeled as proof of these counter/identity changes.

## Focused result

- Five Rust graph/membership tests pass, including nested direct/captured identity,
  old capture missingness, both return counts and shared cursor iteration.
  `/tmp/loo291-parent-flow-identity-rust-final.log`.
- Two actual isolated CLI tests pass: pinned topology/counters/rejected restart
  and bound standalone Flow launch/context/Session naming/membership. The latter
  exposed a stale assertion on `membership.current`; updated it to the actual
  occurrence enum and required exact node.
  `/tmp/loo291-parent-flow-identity-cli.log`.
- Sixteen Swift DTO/state checks pass in the first run; the native check failed
  only because my relative capture path resolved under swift and did not exist.
  With an existing absolute output directory, the focused Flow command passes
  three tests in two suites including the complete native PTY proof.
  `/tmp/loo291-parent-flow-identity-{swift,native}.log`.
- All-target clippy, cargo formatting and diff whitespace pass.
  `/tmp/loo291-parent-flow-identity-clippy.log`.

The first Rust compile also found two remaining membership constructors (standalone
Flow projection and managed Flow test expectation); both now use the shared node
key. Original compile/capture failures remain in their logs.

Inspected the replacement native capture in `parent-flow-evidence/`: two arrow
counts no longer manufacture different iterations. This is the synthetic graph
fixture, not the complete expanded builtin Feature or configured provider proof.
The prior cycle-4 screenshots and hashes retain historical attribution. Hosted
Session UI was not run; the fixture correction has compilation evidence only.
