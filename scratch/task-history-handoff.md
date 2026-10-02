# Task history implementation handoff · 2026-10-02

## Feedback and direction

Jack Heart's existing control conversation relayed his request to get LOO-369
running and ready to demo. Coordinator guidance clarified that Show completed
must exclude canceled/duplicate rows. This is coordinator guidance, not a direct
new statement attributed to Jack. The earlier question about including canceled
history is superseded; no answer to it is required.

## Design changes

The [working design](keep-current-tasks-visible-and.md) now limits Show completed
to successful completions. It preserves canceled history in the full shared
inventory and Linear and preserves unresolved work and existing workspace/Session
access. The timestamp pipeline only needs completed_at. A single ordered visibility
rule preserves current/unresolved work before applying terminal-history filtering.
The acceptance wording now explicitly says tests and builds remain pending.

## Evidence and remaining work

The design's Findings and cause section records the earlier snapshot evidence:
all seven duplicates already had canceled state. This review confirmed that
[rust task_summary](../rust/loopflow/src/lf/commands/waves.rs) drops state and that
[TaskFlowGate](../rust/loopflow/src/ops/task_flow.rs) uses completed alone for
unplaced admission. [WorkspaceProjection](../swift/LoopflowMac/WorkspaceProjection.swift)
also uses completed for its working set. These consumers must change together.
Equal-revision timestamp enrichment must distinguish an absent persisted key
from an observed null; the existing branch_name fallback is not that proof.

Next useful action: implement the shared state/completion-time pipeline, then
connect the Wave filter and matching counts, with the cross-boundary cancellation,
time-window and retained-access proofs in the design. No design question blocks
that work. Routine choices are recorded in [questions.md](questions.md).
No implementation, test pass, deployment, merge, Task completion, final demo
approval or Flow navigation decision is claimed by this review.

Check: source/design review only — no production edits; focused tests and app build belong to implementation, affected acceptance to gate, visible judgment to demo/review.
