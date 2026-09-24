# Inline Watch model compression — 2026-09-24

Reviewed `01148246b` and the working navigation/rendering tests and README.
No further coherent model or API reduction found. This pass changes only this
report; the existing implementation and working edits remain intact.

## Model before and after

Unchanged: FlowPosition owns execution; committed Task events retain flow
history. Rust projects plans, attempts, transitions and attributed Runs into
TaskWatchSnapshot, and separately pages journal/native output in TaskOutputPage.
RegistryQuery decodes those contracts. WorkspaceNavigation retains each visited
Task's Watch presentation within its window/repository. TaskWatchStore owns the
last snapshot, error, selection and request identity. The mounted TaskWatchView
owns its cancellable read; retained terminal surfaces keep their existing owner.

No command, DTO, field, type or public API was removed in this pass. The inline
implementation already removed WorkSurfaceView's Watch sheet, sheet selection,
terminal-store reference and selection's Identifiable conformance.

## Path and mirrors checked

Read TaskFlowEvent/TaskFlowStage and the Watch/output projection contracts;
compared their fields with Swift TaskWatch.swift and TaskOutput.swift and the
two shared JSON fixtures. Inspected Rust/Swift fixture assertions, CLI Watch and
Output declarations, RegistryQuery's typed reads and temporary cursor transport,
and RegistryQueryLocal. Followed WorkspaceProjection/Navigation through
WorkspaceNavigator, WorkSurfaceView and SessionsView into TaskWatchStore/View;
checked the remaining TaskWorkspaceView destination and focused navigation
tests. Read the updated Swift README and prior proof receipts. Negative searches
found neither TaskOutputRecord nor TaskWatchStepKind in source or Swift tests.
The earlier report below retains the deeper launcher review's separate scope.

## Candidates retained

- The navigation dictionary and sheet-local Watch store have different view
  lifetimes. They instantiate the same presentation model. A global cache or new
  shared navigation wrapper would add ownership and couple independent views.
- Completed Task visibility belongs to navigation. The projection now retains
  every planned Task; the toggle, search and selection decide visibility without
  another inventory. Removing the toggle loses an explicit user choice.
- Snapshot/error and selection/request identity remain independent: stale
  evidence survives failed reads, historical selection survives refresh, and
  late responses cannot replace newer evidence. Derived invocation, stage and
  isRefreshing already avoid duplicate state. TaskFlowStage includes iteration,
  whereas UI stage selection deliberately includes all attempts.
- Runs cannot collapse into attempts: auxiliary Runs have no stage, and bound
  attempts can outlive readable manifests. Output attribution makes each page
  interpretable independently of the snapshot. Source health, reset and paging
  are distinct observations, not aliases for an empty records array.
- Snapshot and output reads serve different evidence and continuation needs.
  Combining them would not solve unbounded discovery or independent history/live
  continuation. Those remain required implementation work, not removable APIs.

## Verification and remaining scope

No tests or builds rerun because executable content is unchanged. Inspected the
prior 18-test pass in `/tmp/loo293-unified-watch-tests.log` and the subsequent
two-case standalone/inline rendering pass in `/tmp/loo293-watch-inline-render.log`.
These remain local implementation receipts, not fresh compression validation or
configured Watch proof. `git diff --check` passes.

Complete capture, bounded discovery/state, independent history/live continuation,
the feed, filtering/Follow live, human Session navigation and the configured demo
remain required in this same Task and PR. This review establishes no Task
completion or publication readiness.

---

# Earlier Watch UI model compression — 2026-09-23

No executable reduction taken. This extends the foundation review in
`compress.md` to the Mac inspection slice saved at `021fc6470`. A concurrent
rebase started during inspection; later reads used that immutable checkpoint.
This report does not assess the rebased result or resolve its conflicts.

## Model before and after

Unchanged: FlowPosition authorizes execution; committed Task facts retain plans,
attempts and transitions. Rust projects those facts and Run attribution into
TaskWatchSnapshot. Native history and journals supply the separate grouped
TaskOutputPage. RegistryQuery decodes both. TaskWatchStore retains one snapshot,
its last error, selection and request identity. TaskWatchView derives presentation
from them; the existing launcher owns cancellation of its query subprocess.

No DTO, command, field, type or public API was removed or collapsed.

## Path and mirrors checked

Compared Rust `ops/task_watch.rs` and `ops/task_output.rs` with Swift
`TaskWatch.swift`, `TaskOutput.swift`, and both shared JSON fixtures. Followed
RegistryQuery through RegistryQueryLocal and MacLocalWaveAgentLauncher. Read
TaskWatchStore, TaskWatchView, TaskWorkspaceView, the Work/Roadmap/Wave entry-point
diffs, TaskWatchTests, and the Swift README. Searched the checkpoint for obsolete
TaskOutputRecord and TaskWatchStepKind names; neither remains.

## Candidates left intentionally

- **Snapshot and selection:** invocation and stage are computed from one snapshot,
  not copied into a second flow model. Invocation selection can exist without a
  stage, including an earlier invocation with missing plan evidence. Replacing
  the two selection fields with a new enum would add vocabulary and bindings
  without deleting an owner. Reusing TaskFlowStage would incorrectly require an
  iteration for a selection that shows every attempt.
- **Refresh trigger and request identity:** the view trigger restarts SwiftUI's
  cancellable task; the store identity rejects late results from overlapping
  refreshes. isRefreshing is already derived. Combining them would couple view
  lifetime to result ordering rather than remove duplicated state.
- **Snapshot and error:** both must coexist to show last-good evidence after a
  failed read. The error cannot be encoded as an empty snapshot or inferred from
  missing flow receipts.
- **Cancellation and output collection:** one synchronizes spawn/termination;
  the other drains stdout/stderr concurrently. Both stay inside the existing
  launcher. RegistryQueryLocal's former detached wrapper is already removed;
  there is no second Watch process runner to consolidate.
- **Run labels and attempts:** provider labels come from the snapshot Run list;
  attempts retain exact Run IDs. Adding labels to each attempt or a Swift join
  cache would duplicate evidence. Auxiliary Runs justify the independent list.
- **Workspace and entry points:** all three entry points use the same existing
  workspace and Watch view. Their sheet selection belongs to their navigation
  owner. A new shared navigation wrapper would save spelling, not a concept.
- **Shared evidence fields:** settlement, attempt state, active coordinates,
  source health, reset and pagination describe different facts. A field unused
  by this first view, such as blocker restartRequired, still belongs to the
  shared core contract; removing it only from Swift would create mirror drift.

## Verification and boundary

No tests or builds rerun: executable behavior is unchanged. The prior five
focused tests, Mac build and rendered fixture remain prior receipts. Only this
report was added; the concurrent rebase and its index were left to their owner.
Full output capture, bounded discovery, independent history/live continuation,
the feed, Session navigation, Follow live and the configured human demo remain
required in this Task. This pass establishes no publication or landing readiness.
