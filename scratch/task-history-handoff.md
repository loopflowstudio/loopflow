# Task history implementation handoff · 2026-10-02

The reviewed LOO-369 change is implemented locally. Rust preserves provider state
and actual completion timestamps through planning storage, Task summaries and
roadmap. Canceled/duplicate unplaced Tasks stay in the inventory with terminal
reasons, Later placement and unavailable start controls. Equal-revision enrichment
permits a completion date only when its stored key was absent; observed null,
conflicting dates and older revisions retain their existing protections.

The production Wave and alternate Wave detail share hidden/7-day/positive-N/All-time
filtering, truthful row counts and validation. Current planning, unresolved Flow
execution and observed unsettled work remain visible. Session membership and
workspace inventory remain independent. Missing planning no longer turns local
abandonment into successful completion.

Jack Heart relayed the coordinator finding about completed Tasks whose removed
worktrees produced stale recovery conditions. Completed + done + missing checkout
+ recovery_required now remains subject to the history window. That regression,
retained Session access, unresolved review, reopened planning, subsecond boundaries,
UTC offsets and DST crossing have focused coverage.

The shared synthetic PM scenario feeds Rust provider validation, storage and
projection. Its checked roadmap rows are read through RegistryQuery into the
production Wave view; control interactions assert visible identities, labels and
counts. The unresolved runtime variant and retained Session proof are simulated.
No configured Growth refresh, live provider write, installation or visual approval
is claimed.

Remaining: affected gate suites, configured app build, then the authored Desktop
demo/review. The existing design retains that command plan and demo scenario.
Publication, landing and Task completion remain outside this implementation step.

Compression removed the unconditional ready-to-start action fallback. Unplaced
Tasks now reuse their projected next-move reason, including cancellation and
completion. Swift history predicates no longer depend on display labels; tests
load individual rows without assembling a roadmap except for the view scenario.
The implementation checkpoint is `e2a9277b0`; compression remains in the diff.

Check: `cargo test -p loopflow --lib task_history` — 3 passed; `swift test --package-path swift --filter 'TaskHistoryFilterTests|RoadmapViewTests'` — 7 passed, SwiftPM app compiled; `cargo fmt --check` and `git diff --check` — passed; `cargo clippy --all-targets -- -D warnings` passed on the implementation checkpoint, final affected suites/Clippy/configured build to gate, visible judgment to demo/review.
