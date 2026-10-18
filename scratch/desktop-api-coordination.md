# LOO-497 — Desktop integration contract

## Accepted direction — October 17

Jack Heart accepted the prototype and the distinct Dashboard correction: custom
query/arrangement destinations reuse Work-owned Task tools and never replace the
ordinary repository/Wave Console. One Shell and one Files tool per Task contain
independent tabs. No higher-scope tools, tear-out or duplicate document viewers.
The complete prior contract is checkpointed at
`16c1b19c92:scratch/desktop-api-coordination.md`; the materialized 489 source and
its attribution remain in `native-console-integration.md`.

## Integrated owners

- Rust `ViewPlan` owns grammar/evaluation, attention, ordered occurrences,
  first-visible-wins and retained arrangement. `PlanningPart.selections` carries
  built-in and Dashboard plans; Swift never evaluates membership.
- `DashboardId` owns independent name/query/pin and expression-specific geometry.
  Shared CLI/API `query`, `rename`, `pin`, `arrange` enforce saved revisions.
  Native sidebar/editor/Find Work use those same operations. `view --plan` is
  passive; live `view` routes through the existing repository window link.
- `SessionsWorkspaceRegistry` owns one native workspace per Work/window. Native
  Dashboard tiles borrow it and use primary Session unless explicitly zoomed.
  Full retained trees keep hidden members while edits change visible neighbors.
  Suppressed duplicates retain their membership and winner in inspection.
- `SessionsWorkspace` adopts portable read/save at application entry. Delayed
  reads merge missing content into current tabs/selection/geometry/drafts.
  Serialized saves advance revisions without replacing live content. Conflicts
  stay visible and require explicit reload; writes are never blindly replayed.
- `MultiplexerStore` owns pane/tab identity and exact native input/read targets.
  Matching restore content preserves incarnations; cold Shells have no command.
  `TaskFilesStore` owns Task/path documents across tab close; recovered drafts
  reconcile their base against disk. Other-location recovery text is retained.

Normal Console working-slot order remains independent from query membership;
paused Tasks leave working slots but stay searchable and queryable. Repository
shows its conversation; Wave shows its conversation and current Task inventory.
The earlier plan to replace Swift working-slot predicates with query defaults is
superseded by Jack's correction. Exact pane hide/focus/move operations retain
native target fencing; Dashboard occurrence operations use full revisioned maps.

## Delete — do not maintain

Removed: default-view bindings, SavedSelection identity, split-as-shell commands,
pane-keyed shell caches, duplicate Files owners, old Console `ViewPlan`/`ViewMember`
and separate selection stream, leaf-dropping outer hide. Their required data and
behavior survive in Dashboard/Work intent, shared plans and retained native trees.
Existing direct opening remains the sole Session/Changes readiness path; Dashboard
links join it at the repository receiver and do not acquire a provider.

## Verification boundary

Focused production tests cover Dashboard/default navigation retaining the Task,
shared CLI-plan/native membership, exact tabs, delayed two-shell/two-file restoration
with newer tabs and drafts, stale-write atomicity, later saves, cold restore without
launch arguments and retained Dashboard occurrences while other splits change. Final command
results live only in design.md.

Gate still owns candidate-CLI/Swift combined acceptance with a disposable registry:
started running/paused/idle, backlog/finished Tasks, empty/child Waves, repeated and
nested queries, incomplete evidence, duplicate promotion, invalid map atomicity,
departure versus unavailable retention, pin/rename propagation and Linux planning.
Use actual shared readers; mock external process/network effects only.

Jack Heart resolved the composition question on October 18: side-by-side Tasks
appear only as query results, and queries are shown only through Dashboards.
The original generic open-beside acceptance is superseded. Ordinary navigation
now selects one destination; Command-click splitting and the independent outer
hide/resize tree are removed. Saved and ad hoc query links share Dashboard
presentation without requiring every query to be saved or pinned. Dashboard pane
focus preserves the shared projection; only ordinary navigation replaces it. The remaining scripting-owner boundaries are in
`design.md`.

LOO-416 owns Process reattachment. Current native Shell launch does not capture
an lf_process_id in its tab and cold intent has no transport admission handle.
It restores unattached without replaying commands. Merge `7bfa10fa72` /
`2523534b29` describes session/transport attachment, but this snapshot is not
combined native continuity evidence. Saved records confer no lifetime authority.
Cross-machine mapping and mounted quit/relaunch continuity remain explicit demo/
transport evidence. Publication is for review; no merge or completion is authorized.
