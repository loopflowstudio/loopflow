# LOO-497 — Dashboards and shared Desktop APIs

## Accepted direction — October 18, 2026

Jack Heart accepted the query-view prototype for implementation on October 16.
On October 17 Jack specified a distinct Dashboard type (name provisional) that
reuses Task workspaces, and excluded replacing repository/Wave defaults. That
correction supersedes the earlier Work/default-view binding proposal. Jack also
accepted one Shell pane and one Files pane per Task, with independent tabs inside
each tool; the detailed contract below supersedes shell-per-pane creation.
On October 18 Jack Heart clarified: for now, side-by-side Tasks appear only as
query results, and queries are shown only through Dashboards. This supersedes the
original generic `open <ref|query>` beside-current-selection requirement. Ordinary
navigation selects one destination; do not add an outer split between destinations.
Ensure live query entry points use Dashboard presentation, including an ad hoc query;
this decision does not require saving or pinning every query.

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
- Waiting Tasks sort first; the implemented 2:1 automatic area remains a provisional
  weighting choice. Structural edits
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

The integrated Console source snapshot is preserved at `8d4f2d8ea1430fd41d946d328145ce87e26c7b7b:scratch/history/loo489-console-source-2026-10-17`.
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
nor deleting a Dashboard owns or deletes the Task's content. Swift SessionsWorkspaceRegistry/MultiplexerStore remain the live owners. Native destinations
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

Removed (rationale and review findings: `08253c4dda:scratch/design.md`). Do not
reintroduce:

- Outer split between destinations: `besideFocused`, Command-click splitting,
  `SessionsWorkspaceRegistry.hide`/`resize` and its hidden tree,
  `WorkModel.hideWorkspace`, `WorkLayoutView`, the `WorkLayout` mirror.
  Window geometry is readback only; Dashboards arrange via the revisioned writer.
- Bindings: `SelectionBinding`, `binding_in`, `default_owner`, `selection_binding`,
  `bind_selection`, `DesktopCommand::Bind`, Swift binding DTO, `work_selection_bindings`.
- Earlier: `desktop list`, Desktop's re-export of Work identity, the pane revision
  column, old `ViewPlan`/`ViewMember` and the separate selections stream
  (`PlanningPart.selections` owns plans), the `selection_plan_in` wrapper.
- Shell-per-pane: `MultiplexerStore.split`, `SessionsWorkspace.split`,
  `newShell(beside:focus:)`, pane-keyed `shellCommands`, `retainedShells`,
  `DesktopCommand::Split`/`DesktopPaneAction.split`, synthetic `PaneState` in inspection.
- Duplicate `WorkArrangementTree.Axis` (use `SplitAxis`).
- The repo-name chevron that opened Find Work (October 19; jump bar replaces it).

Kept on purpose: Console working-slot predicates differ from Dashboard membership;
exact-pane hide/focus/move commands and revisioned `arrange` are distinct scopes.
Pane ownership reads `tasks.repository_id`; leaving a Project keeps saved panes.
Saved and ad hoc Dashboards share `selectDestination` (history, palette dismissal).
Validation covers only the edited selection and its references.

## Implemented native boundary — October 17

Separate Dashboard destinations, pinned discovery, Cmd K entries and name/query/
pin editing consume Rust plans. Tiles reuse retained Task workspaces, show their
primary Session or explicit zoom; unstarted Tasks offer Start (agreed-ux.md). Hide/restore, drag
swap and split resize edit the full saved occurrence tree; resize commits once
per gesture. Ordinary repository/Wave views retain the Console contract.

Live `lf view EXPRESSION` opens the existing repository URL route; `--plan` stays
headless. Native inspection reports the selected full plan, including suppressed
occurrences. Opening generations fence late reads. Named plans refresh through
the planning frame; unsaved queries re-read the shared planner when it changes.
The CLI result schema includes passive plans and live opening receipts.

## Reconciliation — October 18

Baseline `2839e623d9` (v0.14.6, LOO-505 optional Task membership). Saved panes
follow the Task's repository. Product memory's upstream LOO-505/LOO-503 sections
were restored. The anonymous-query save-before-arrange choice in `questions.md`
remains an implementation assumption, not accepted removal of arrangement parity.

## Remaining implementation, verification and transport

- Demo preparation: see [desktop-dashboard-demo.md](desktop-dashboard-demo.md) for
  the clone temp-path/disk-budget repair and candidate CLI↔native targeting gap.
- Scripting parity stays with existing owners: readiness/events and wait/run-wait
  with LOO-453 (and the new scripting Task, `questions.md`); LOO-498 owns external
  hosts. Opening receipts report launch acceptance, not provider readiness or exit.
- LOO-416 cold Shell attachment: native launches record no `lf_process_id` on the
  Shell tab and restore has no admission handle, so restored shells stay unattached.
  No provider launch, lifetime registry or saved attachment authority was added.

## Acceptance and review

Focused coverage proves: independent Dashboard identities, immutable built-in
defaults, unchanged pane/draft ownership, distinct occurrence roots for identical
expressions, passive readback, revision conflicts, released-frontier schema
upgrade, Rust/Swift DTO round-trips, two shells/two files retaining identity,
order, selection and drafts through restore, background reads not following
selected tabs, invalid writes changing no revisions, delayed restoration keeping
newer tabs/drafts, cold shells without commands, one Task owner across Dashboard
and default navigation, and (October 19) the unstarted Start offer and jump bar.
Review repairs are recorded at `08253c4dda:scratch/design.md`.

Gate owns affected suites and combined headless native/Linux acceptance
(`desktop-api-coordination.md`). Demo judges rendered continuity: Wave → Dashboard
→ hide A → resize B → enter A/draft → restart/restore A, Wave default unchanged.
Unavailable Work must not appear deleted; failed saves stay visible.

Check (October 19, implement): `scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkDestinationTests|DashboardWorkspaceTests' -Xswiftc -gnone` — 94 passed; adjacent navigation/Session/workspace suites — 49 passed. Broader suites stay gate-owned.

## Handoff status

Earlier design/readback: `8d4f2d8ea1:scratch/history/loo489-console-source-2026-10-17/loo497-before-continuation-design.md.txt`.
October 17: Jack Heart rejected the prototype's invisible aliases
(`query:product or query:available`); the study uses direct predicates. This sets
no production query-reference syntax.
