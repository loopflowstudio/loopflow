# LOO-489 implementation findings — October 17, 2026

Jack Heart approved the Console hybrid and Swift demo. Historical design holds,
grids, favorites, attention sorting and the Wave footer are superseded. Complete
pre-implementation notes and dirty source are preserved at
`refs/loopflow/evidence/loo-489-before-console-implement` (tree
`4c6c6cc9cfaceb3387af16139b5dd5944f9768ff`).

Native tabs use the operator's bounded native-tool-handoff.md snapshot.
Public APIs and Dashboard composition remain LOO-497's responsibility.

Reversible choice: seed working order from already-open retained Task references,
then append on explicit started/unpaused Task selection. Query arrivals alone add
nothing. WorkCache saves these references without restoring selected Work or
acquiring Sessions. Missing planning retains the reference; explicit pause hides
it without deleting its relative position. Runtime/planning aliases share one slot.

Remaining implementation and native acceptance are in navigation-implementation.md.
No publication, merge or Task completion occurred in this implement step.

October 18 integration: checkout update had preserved the accepted implementation
and design in stash `55548763c7` (untracked tree `61a15bcab5`). Reapplying that
stash was the reversible recovery choice; it remains intact. Conflicts were
resolved against conversation entry and LOO-505's standalone Task ownership.

Current LOO-505 integration, accepted editor removal and native acceptance gaps are reconciled in
navigation-implementation.md. Earlier findings remain at
`34955867e280f1fa523ce32337775cbb740c5302:scratch/questions.md`.

October 17 remainder: Wave overview reuses roadmap objective/status/project data
and existing Project history, realignment and Workflow inspectors; no extra reader.
Sidebar search means the currently displayed Task rows plus direct Wave rows;
Search all Work restores general navigation. Shift-F10 presents shared Task actions
in a keyboard-dismissable popover. Full pre-curation notes are preserved at
`9f4574310e03f73ae4d7b381ceac9da10e5251b5:scratch/`.

Recovery choice: missing provider history remains an explicit, recoverable error;
never treat an absent transcript as an empty conversation. Preparation reads it
before Desktop releases the terminal. Retry follows a saved first turn.
