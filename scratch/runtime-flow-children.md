# Runtime loop ownership

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
