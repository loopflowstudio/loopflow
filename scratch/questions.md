# LOO-1108 open choices

- Assumed: nothing replaces verify's post-merge check (post-merge CI for the merge SHA, claims against main, fix forward). Passed over: a post-merge step inside `deliver`; keeping `verify` as an unreferenced builtin.
- Assumed: the outer loop in `code`/`feature` goes with verify (a loop cannot end in `cmd: land`).
- Assumed: `lf task run`'s `merges:` line is driven by `CapturedGraph::reaches_landing` (using `landing_name`) rather than `TaskRunSelection::reaches_end`, so non-merging Graphs (`pursue`, `deep-clean`, etc.) stop warning that they may merge and custom post-merge-stop Graphs warn honestly.
- Deferred outside LOO-1108: moving Swift's `StartFlow.merges` heuristic onto a `WorkflowOutline` wire field (requires a DTO schema change).
- Deferred outside LOO-1108: hand edits in other repositories on this Machine (`manabot`, `fantasia`, `etude`, `etude-sdk`) that name `verify`.
