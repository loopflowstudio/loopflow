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

The earlier affected gate passed: 145 Rust tests (two intentional ignores), 68
Swift tests and the final five-test history suite, plus formatting, Clippy,
architecture, Swift boundaries and configured Xcode compilation. It closed
intermediate-window exclusion and production Start-action assertion gaps.

Jack Heart then approved the inline checkbox/number prototype. The Desktop
revision uses Completed / N Days / All Tasks, with zero meaning unbounded.
One native text surface preserves glyph placement during editing; long integers
scroll within the compact viewport. Compression removed duplicate editability
assignments and repeated row reads. Seven focused tests and SwiftPM app
compilation passed after compression.

Jack's Growth screenshot shows six current Tasks without canceled duplicates.
The prepared offline demo contains those thirteen provider items but no successful
completions or Sessions. It cannot prove live refresh or Session continuation.
The matching packaged source CLI makes an installed-runtime upgrade unnecessary
for that isolated demo. See [durable evidence](../docs/reviews/task-history.md).

Remaining: queue gate for the inline revision, native appearance/focus review,
live Growth refresh and retained workspace/Session continuation. Publication,
landing and Task completion remain with the caller. The
[current plan](keep-current-tasks-visible-and.md) owns acceptance and scope.

Check: `swift test --package-path swift --no-parallel --filter TaskHistoryFilterTests` — recorded 7 passed after compression, SwiftPM app compiled; reconciliation changes prose only and reuses this result.
