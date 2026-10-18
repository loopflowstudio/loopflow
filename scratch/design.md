# LOO-497 — Dashboards and shared Desktop APIs

## Accepted direction — October 17, 2026

Jack Heart accepted the query-view prototype for implementation on October 16.
On October 17 Jack specified a distinct Dashboard type (name provisional) that
reuses Task workspaces, and excluded replacing repository/Wave defaults. That
correction supersedes the earlier Work/default-view binding proposal. Jack also
accepted one Shell pane and one Files pane per Task, with independent tabs inside
each tool; the detailed contract below supersedes shell-per-pane creation.
Publication is for demo review; merge and Task completion remain separate.
Complete pre-compression notes are preserved at
`refs/loopflow/evidence/loo-497-before-intent-compress:scratch/design.md`
(tree `bbb68ebbc97d0b304806aa564333651d77629a2d`).

Pre-cut code, complete design, contrary evidence and earlier check commands are
preserved at `refs/loopflow/evidence/loo-497-before-dashboard-cut` (tree
`d68851b7807858609266546ea8d459a4ebf7fc17`). Earlier foundations design remains at
`refs/loopflow/evidence/loo-497-foundations-design` (blob
`e737550f687c0d5f17c363e1ebe2c00a31f8fcde`). Historical proposals there do not
supersede the October 17 correction.

## Contract

- Repository is implicit window scope, never a query predicate. Repository/Wave
  built-in selection plans select started unfinished Tasks throughout descendants
  and immediate child Waves, including empty Waves; these are not the native
  repository/Wave presentation. LOO-489’s Console contract below owns that UX.
  Paused/idle/waiting Tasks remain queryable. Explicit
  queries may include backlog/finished Tasks; an unstarted Task shows its brief
  without acquiring a Session or execution.
- A Dashboard has independent identity, name, selection and arrangement. Two
  Dashboards may select identical Work. Query text describes membership, never
  Dashboard identity. Task panes, Sessions and drafts retain their Work owner.
- Rust owns the bounded wave/task/query/kind/state grammar, implicit AND, OR and
  grouping, evaluation, attention and first-wins projection. No Swift evaluator.
  Authored clause IDs survive sibling insertion, repeated clauses and value edits.
- Retained binary trees own axes, ratios and hidden members. Full replacement
  requires every occurrence exactly once, including hidden members. First visible
  Work occurrence wins in tree order; focus cannot choose the winner. Hide/restore
  retains current edits and promotes duplicates without creating content.
- Waiting Tasks sort first and receive twice the automatic area. Structural edits
  freeze order/proportions; arrivals append. Incomplete reads preserve existing
  members/geometry; confirmed departures prune. Missing refs are unavailable
  evidence; cycles, invalid IDs and invalid layouts are errors.
- lfdb owns Dashboard metadata, expression-keyed arrangement history, and portable
  Work pane/file-draft intent with revision-checked writes. No result cache, native
  handles, attachment claims, window incarnations or inferred liveness. One Task
  migration draft, `saved_selections.sql`, is edited in place.
- Tile face is explicit zoom, otherwise primary Session, never ordinary focus.
  Missing primary/location remains explicit. Saved intent grants no attachment or
  execution authority. Draft records retain Machine/path/base/text; saving writes
  no file. Native restore must compare the base with current disk.
- Use LOO-489’s October 17 Console: stable working Task sidebar, direct Wave rows,
  repository conversation, Wave conversation beside all current planned/started
  Tasks, one titlebar, and Cmd K Find Work. Explicitly paused Tasks stay in search
  and Wave inventory while leaving working slots; query membership is independent.
  Dashboards are additional destinations and never replace those default surfaces.
  Task Shell/Files remain single tools with tabs. No Shell/Files tools at higher scopes.

## Current prototype and integration direction

Jack Heart requested rebasing/integrating LOO-489 and rebuilding the UX prototype
from its latest work on October 17. `scratch/query-view` now contains a local
snapshot of 489’s Console study plus separate Dashboard composition. The full
accepted navigation contract is copied into `scratch/navigation-console-contract.md`;
source SHA256 hashes are in `scratch/query-view/source-manifest.json`.

The revised study explores Dashboards below Waves, separate name/query editing,
Task tiles referencing existing drafts/tools, drag-to-swap, split resize and
hide/restore. Product watch demonstrates overlapping queries; explicit unstarted
Tasks show briefs. The study adds no production lifecycle or query evaluator.
These new Dashboard controls are reviewable proposals, not a completed native demo.

Console source is captured under `scratch/history/loo489-console-source-2026-10-17`.
The original 497 work is checkpointed at `16c1b19c92`; committed Console ancestry
is merged at `6e63af826e`. Snapshot changes are reconciled through existing owners.

## Owners and implemented boundary

`work::selection::Dashboard` / `DashboardId` replace `SavedSelection` / `QueryId`
as the custom-view entity and identity, without a wrapper or second store.
`Selection` remains the expression and `query:ID` references a Dashboard's query.
`DashboardReading` joins saved intent with current Work evidence. The store's
`edit_dashboard`, `save_dashboard`, `read_dashboard`, `dashboards`, metadata and
revision-checked deletion operate on `dashboards` / `dashboard_arrangements`.
Expression hashes exclude authored IDs; revisits recover their IDs and layout.

`ViewPlan.dashboard_id` distinguishes Dashboard plans from built-in defaults and
ad hoc expressions. The existing Work observer supplies defaults and Dashboard
plans in one planning frame. Swift `Dashboard`, `WorkViewPlan`, and `RegistryQuery`
consume the shared contract. Dashboard mutations use the same `query:ID` selector
as planning; bare IDs remain valid. Names/pins and Task rename use shared writers.

CLI `desktop state` inspects actual native geometry/identity. Headless `view --plan`
uses the existing Work reader without creating a registry or acquiring execution.
`query`, `pin`, `unpin`, `rename`, `arrange`, and `layout` edit portable intent with
revision checks. Invalid syntax exits 2; missing/ambiguous selection exits 3.
Plans include Work/title/current states, primary Session, saved pane revision/face,
recorded Machine/cwd and the existing connect argv. Draft bodies stay on explicit
layout reads rather than repeated observation plans.

`work_pane_layouts` remains keyed solely by Work. Independent Dashboards resolve
the same pane revision/face and recover the same Task draft record; neither hiding
nor deleting a Dashboard owns or deletes the Task's content. Swift WorkLayout/MultiplexerStore remain the live owners. Native destinations
consume the shared plans; navigation and provider lifetime keep their existing owners.

The accepted tab payload now spans Rust `PaneIntent`, Swift `WorkPaneIntent`,
fixtures and native tool state. Shell contains ordered `{id, location,
lf_process_id}` tabs and selection; Files owns its recorded location plus ordered
`{id, path}` tabs and selection. The shared writer atomically rejects duplicate
tools/documents/IDs, missing selected tabs and non-Task tools. File drafts stay on
Work, independent of tab visibility. An empty Files tool retains its location.
Jack accepted one visible shell and one ordinary document per tool; no tear-out, docking, tab groups or duplicate viewers.
A specialized diff may show both sides inside Files. Closing a file tab retains
its Task/path document and draft. Dashboards reuse these Task tools without
inferring Task ownership from their focused tile.

`MultiplexerStore.newShell` adds/selects a retained tab; `ShellTab.id` owns terminal
identity independently of its pane. Native inspection includes ordered tabs and
selection. CLI `--pane shell:ID` (or raw ID) captures that tab's incarnation and
surface, including background tabs. Receipts return `pane` and `tab`. Non-Task
Shell requests fail before mutation. Tool selection preserves other tools; one
Task Files store handles aliases and keeps document drafts/Undo through tab close.

`MultiplexerStore.paneLayout`/`restore` and `TaskFilesStore.paneIntent`, `drafts`,
`restoreTabs`/`restoreDrafts` provide the native conversion boundary. Newly
reconstructed shells have fresh incarnations and no launch command; they display
unattached. Matching shells already in memory retain their runtime state.
Recovered drafts remain read-only until the existing disk reconciliation runs.
The application now loads and saves this intent through `SessionsWorkspace`.
A delayed read merges missing content into newer geometry, tabs, selection and
drafts. Saves are serialized, debounced and revision-checked; responses advance
only the revision. Conflicts retain live edits and durable state, display an error,
and offer an explicit reload. Drafts reconcile with disk before becoming writable;
unavailable locations retain their recovery text. Matching native pane and tab
objects retain their incarnations. Cold restore carries no shell launch command.

## Delete — do not maintain

Removed in the Dashboard cut: `SelectionBinding`, `binding_in`, `default_owner`,
`selection_binding`, `bind_selection`, `DesktopCommand::Bind`, its dispatch/schema,
Swift binding DTO/callers, and `work_selection_bindings` with its revision triggers.
The existing schema draft directly describes the final shape. Exclusive binding
assertions are replaced by proof that Dashboard discovery/editing leaves every
built-in default unchanged. Saved custom arrangements and Work drafts remain.

Previously removed: `desktop list` dispatch/schema/docs, Desktop's compatibility
re-export of shared Work identity, and redundant pane revision column (the pane
record remains the revision authority). These cuts are retained.

Compression removes old-query evaluation before editing and catalog-wide
evaluation on save. Validate the replacement and its references: any new cycle
must include the edited selection. An unrelated ambiguous Dashboard must not
block another edit, and an ambiguous query must remain repairable. The extra
`selection_plan_in` wrapper is removed; built-in defaults enter `plan_in` directly.
Session summaries are grouped by Task once when reading selection facts.

Removed tab predecessors: `MultiplexerStore.split`, `SessionsWorkspace.split`,
`newShell(beside:focus:)`, pane-keyed `shellCommands`, `retainedShells`,
`DesktopCommand::Split` / `DesktopPaneAction.split` and exclusive allocation tests.
Existing moves and Session geometry retain arrangement coverage.

Query edits now use the same saved-intent CLI dispatcher as other Dashboard edits,
sharing registry opening and selector normalization. Swift portable trees use
`SplitAxis` directly; the duplicate `WorkArrangementTree.Axis` and conversion
branches are removed. Shared DTO tests reuse one fixture loader. Review corrected
the receipt documentation: Work identity is returned; Machine/checkout are read
from inspection, not invented receipt fields.

The tab compression removes synthetic `PaneState` values from inspection and exact
shell targeting. Input/read resolve a retained tab's terminal once; visibility uses
the retained pane. Tool-only targets cannot follow selected tabs into input. The
single Files owner drops unused Task-ID parameters. Pane ownership reads now join
Work directly, reusing deletion visibility and retired-Wave rules without projecting
unrelated Task/Session activity; validation
borrows pane/tab IDs and handles each content kind once. Review also repaired
surviving fixtures that still treated a pane ID or a newly allocated shell as the
retained terminal identity. Display-only input/continuity fixtures compile; their
runtime checks remain with gate/demo.

Console integration removed the older `ViewPlan`/`ViewMember` and separate
selections stream; `PlanningPart.selections` owns the full shared plans. The
leaf-dropping `SessionsWorkspaceRegistry.hide` is replaced with a retained tree
plus hidden Work. Console working-slot predicates remain: Jack's accepted default
navigation intentionally differs from Dashboard membership. Pane hide/focus/move
commands still operate on exact native panes; Dashboard membership edits use the
revisioned `arrange` API. These are distinct scopes, not competing authorities.

## Implemented native boundary — October 17

Separate Dashboard destinations, pinned discovery, Cmd K entries and name/query/
pin editing consume Rust plans. Tiles reuse retained Task workspaces, show their
primary Session or explicit zoom, and leave backlog passive. Hide/restore, drag
swap and split resize edit the full saved occurrence tree; resize commits once
per gesture. Ordinary repository/Wave views retain the Console contract.

Live `lf view EXPRESSION` opens the existing repository URL route; `--plan` stays
headless. Native inspection reports the selected full plan, including suppressed
occurrences. Opening generations fence late reads. Named plans refresh through
the planning frame; unsaved queries re-read the shared planner when it changes.
The CLI result schema includes passive plans and live opening receipts.

## Remaining verification and transport

Gate owns broader combined native/Linux acceptance; demo owns rendered continuity.
The concrete LOO-416 dependency is cold Shell attachment: existing native launches
do not record an `lf_process_id` on the Shell tab, and portable restore has no
transport admission handle. Restored shells therefore stay unattached. It cannot
establish quit/relaunch or cross-machine Process continuity. No provider launch,
second lifetime registry, or saved attachment authority was introduced.
Publication and demonstration belong to the following authored Flow steps.

## Acceptance and review

Existing focused coverage proves independent Dashboard identities, immutable built-in
defaults, unchanged pane/draft ownership, distinct occurrence roots for identical
expressions, passive readback, revision conflicts, and schema upgrade from the
released frontier. DTO fixtures round-trip in Rust and Swift. Task rename retains
repository-scoped target resolution after deleting the binding reader it used.
The tab cut additionally proves two shells/two files retain identities, order,
selection and drafts through the shared DTO/native restore boundary; reopening a
file reuses its tab/document. Native exact-target tests prove background reads do
not follow selected tabs and removed tabs are fenced. Store tests prove invalid
writes leave both saved layout and store revisions unchanged.

Review repairs: Files aliases could allocate duplicate viewers, so the Task owns
one Files store/pane. Empty Files lost its location, so location belongs to the tool
rather than being inferred from its first tab. Shell exit/navigation still assumed
pane identity; they now resolve shell IDs. Undo renews tab incarnations and removes
launch commands. Session reset reveals a Session without replacing retained shells.
The former simultaneous-two-shell display fixture was deleted with that behavior;
headless two-Session geometry remains covered, and tab mounting belongs to demo.

Review found query mutation treated `query:ID` as a literal ID while planning and
other mutations stripped the prefix. All Dashboard edits now use the same target
interpretation; CLI coverage creates through `query:backlog` then reopens it.

Production application tests now cover delayed restoration after newer Shell/Files
tabs and draft edits, stale writes preserving live and durable state, cold shells
without commands, later edits persisting, and Dashboard/default navigation retaining
one Task owner. Review also repaired hidden outer placement, URL space decoding
and observation refresh revealing a deliberately collapsed primary pane.
Gate owns the affected suites and combined headless native acceptance described
in `desktop-api-coordination.md`, including Linux planning. Demo judges rendered
continuity: normal Wave → custom Dashboard → hide A → resize B → enter A/draft →
restart/restore A, preserving geometry and the same Task workspace while the Wave
still shows its built-in default. Unavailable Work must not appear deleted; failed
saves stay visible and stale writes preserve newer state.

Check (October 17, implement): `scripts/test_desktop.sh --filter 'DashboardWorkspaceTests|ConsoleWorkspaceTests|ToolTabTests|WorkSelectionTests|DTOFixtureTests/desktop|WorkspacePanesTests' -Xswiftc -gnone` — build and 25 tests pass; `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --lib store::sqlite::selections::tests < /dev/null` — 14 pass; same runner with `--test integration cli_discovery::desktop` / `cli_discovery::structured_help_describes_pane_creation_and_read_results_without_desktop` / `dto_fixtures::desktop` — 4 / 1 / 6 pass; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, `lf context` — pass/within limits. Gate owns broader native/Linux acceptance; demo owns rendered continuity.

## Handoff status

The verified local 489 snapshot and current integration directive replace the
previous missing-handoff blocker. Full earlier design/readback is retained in
`history/loo489-console-source-2026-10-17/loo497-before-continuation-design.md.txt`.
LOO-416 attachment and cross-launch behavior still require explicit evidence;
portable restoration alone cannot prove Process reattachment.

October 17 query correction: Jack Heart rejected the prototype expression
`query:product or query:available`. Those names were invisible hardcoded aliases,
not authored Dashboard references. The study now uses direct predicates:
`(wave:product state:open) or state:available`, with no hidden query catalog.
This correction does not establish a new production query-reference syntax.
