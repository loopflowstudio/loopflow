# Navigation model reduction — 2026-09-23

## Model and ownership

Shared `RoadmapSnapshot` and `SessionRecord` remain the planning and human-work
contracts. `PodiumModel` owns their readings, including unavailable/last-good
state. `WorkspaceProjection` derives the typed association and preserves
unmatched Sessions. `WorkspaceNavigation` owns each window/repository's selected
Work, presentation, search and expansion. `SessionsWorkspaceRegistry` retains
that window's existing layouts, native surfaces and Session presentation store.
`SessionsStore` owns opening/error presentation and delegates actions to `lf`.

Before this reduction, selected Work was stored in both PodiumModel and the
per-repository navigation object, synchronized when changing repositories.
Session presentation also exposed repository/Wave/Project/Task scopes, although
all callers used repository scope and Work association already belonged to the
unified projection. An unreachable inspector sheet retained a separate Task
terminal dependency.

Afterward, PodiumModel reads selection from WorkspaceNavigation. Repository
changes invalidate pending Activity independently of whether the saved selection
happens to match. Session presentation takes its repository directly; panes read
it from their existing SessionsStore. SessionScope, its filtering/resolution
methods, scope props through the pane tree, and the unreachable inspector sheet
and singleton observation are deleted. Unused SessionItem label/work/step
aliases are removed; active callers already read the shared record. No route, wire DTO, migration, shared
Session action or terminal-lifecycle behavior changes.

## Review and intentional boundaries

Compared the Rust/Swift SessionRecord and RoadmapTask mirrors and their shared
fixture coverage. Their fields describe durable evidence and remain unchanged.
The pending shared action/display-path contract must still come from LOO-284;
this reduction does not invent it locally.

WorkspaceProjection remains a derived value, not another mutable inventory.
Planning row IDs cannot collapse into durable Work IDs: upcoming Tasks have no
runtime and must retain identity when runtime appears. SessionItem's local
opening/prepared/error state cannot collapse into shared SessionState: native
view preparation and durable human resolution have different lifetimes.
MultiplexerStore still owns layout; the surface pool still owns native views.
Combining them with navigation would couple inspection to terminal lifetime.

The legacy inspector's unused sheet binding never received a selection; removing
it changes no reachable action. Existing Task actions and exact native Session
opening/Move here remain. Repository normalization stays at Session action
construction; panes do not infer ownership from cwd.

## Proof

Focused command passed: 35 tests, exit 0. This covers A/D and repository
retention, exact typed associations, unavailable reads, explicit Move here, and
stale Activity rejection:
`swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
'WorkspaceNavigationTests|SessionsStoreTests|PodiumModelTests'`.
Log: `/tmp/main-view-task-compress-proof.log`. The final removal of three unused
computed aliases is structure-only; final compile recorded separately below.

The existing native surface-creation failure remains recorded in
`navigation-proof.md`. No configured native proof, external-product trial,
measured budget, Task completion, or publication is established here.

Final compile passed (exit 0):
`swift build --package-path swift --target LoopflowMac -Xswiftc -gnone --jobs 4`.
Log: `/tmp/main-view-task-compress-build.log`. No source edits followed this build.
The implementation was checkpointed locally before reduction; compression edits
remain in this checkout for the next lifecycle phase. Nothing was pushed.
