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

Native integration still needs the committed 489 Console source. At this read,
489 has live work and branch head `ac973064c6` predates its uncommitted native UX.
`lf sync <489-branch> --plan` chooses `merge_target` for this protected branch;
use that supported integration path after checkpointing 497 and obtaining the
489 checkpoint, preserving both Tasks’ work. Do not start a competing navigation
implementation. Coordination steer `79a4b081-91db-4e70-9091-0fa14a962356` was saved
for the existing 489 operator conversation. The revised prototype does not depend
on that native handoff and is ready for interaction review.


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
nor deleting a Dashboard owns or deletes the Task's content. Rust store tests prove
portable recovery, not native restoration. Swift WorkLayout/MultiplexerStore remain
the live owners. No native query engine, navigation or lifecycle owner was replaced.

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
`restore` replaces the tree/selection and `restoreTabs` replaces the file-tab list;
only matching shell runtime objects and already-dirty documents are preserved.
These conversions are not a policy for merging late snapshots with newer tabs,
selection or drafts. Shared SQLite round trips are tested, but application
save/load callers and Process attachment are still unintegrated. Local native
retention proves neither quit continuity nor cross-machine restoration.

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

Remaining native predecessors and exact consumer cutovers are listed in
`desktop-api-coordination.md`: leaf-dropping outer hide, Swift
working-set predicates and direct selection mutations. Their replacement depends
on the native owner handoff; no parallel engine is introduced here.

## Remaining work and dependencies

**Current continuation:** Jack Heart said “keep going.” Read
`native-console-integration.md` first. Its verified local 489 source snapshot
supersedes the historical missing-handoff blockers below. 497 now integrates
its Dashboard/editor/persistence callers into the captured Console owners.


1. Integrate the tab conversion boundary through a supported LOO-489/416 code
   handoff. LOO-489 owns application save/load against Dashboard and Work-pane
   records. Pending reads must not overwrite newer native tabs, selected tabs or
   drafts; pending writes must preserve revision conflicts and visible save errors.
   No combined handoff is available. The missing adoption policy is substantial
   application work, not another DTO or snapshot-conversion rewrite.
2. LOO-489 native navigation must consume observation plans and use a shared live
   selection entry point for clicks, deep links and view/open/query. Current `view`
   requires `--plan`; saved Dashboard query edits do not change a live container.
   Window-wide occurrence targeting and first-wins composition remain unimplemented.
3. LOO-416 owns attach-only transport, surfaces and Process reattachment. Restored
   shells must attach without replaying launch commands; actual process/output
   continuity and restart remain unproved. Cross-machine identity/record transport
   and application-level restored file-base reconciliation remain unimplemented.
   The document conversion test covers base reconciliation, not the app caller.
4. Native query-editor UX belongs to LOO-497, integrated through the same live
   selection bridge. Dashboard creation/pinning presentation is still a native
   design choice. The accepted prototype supplies behavior, not a production engine.
5. Complete scripting/readiness integration, exact Session/Changes intents,
   opening generations, scoped input/read fences and headless native acceptance
   remain in the coordination contract. Readiness/events stay with LOO-453;
   program status with LOO-422/415; remote launch with LOO-414.

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

The production delayed-read/stale-save cases in `desktop-api-coordination.md`
remain untested; direct conversion calls cannot substitute for application entry.
Gate owns the affected suites and combined headless native acceptance described
in `desktop-api-coordination.md`, including Linux planning. Demo judges rendered
continuity: normal Wave → custom Dashboard → hide A → resize B → enter A/draft →
restart/restore A, preserving geometry and the same Task workspace while the Wave
still shows its built-in default. Unavailable Work must not appear deleted; failed
saves stay visible and stale writes preserve newer state.

Check (October 17, realign): `git diff --check` — passed; `lf context` — within both budgets; prose-only reconciliation reuses compression's passing `uv run --no-sync python scripts/test_network.py cargo test -p loopflow --test integration cli_discovery::desktop_query_plan_reads_backlog_without_execution_or_desktop < /dev/null` (1), `scripts/test_desktop.sh --filter 'ToolTabTests|WorkSelectionTests' -Xswiftc -gnone` (8), formatting and clippy. Earlier 50 Rust / 76 Swift results and commands remain in the pre-compression reference. Combined native/Linux acceptance remains gate-owned; rendered continuity remains demo-owned.

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
