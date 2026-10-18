# LOO-497 — Remaining Desktop integration

## October 17 concept-review correction

Jack Heart accepted tabs-only for shells and files: at most one Shell pane and
one Files pane per Task workspace. Shell tabs retain independent processes and
identities; file tabs reuse the Task/path document owner. No tab tear-out, extra
tool panes or duplicate viewers. Shell creation is Task-only, excluding repository,
Wave and Dashboard workspaces and superseding the original repo-shell exception.
Adding a shell selects a new tab, with no split direction or anchor. Coordinate
native tool tabs and persistence with LOO-489/416; preserve existing shell state
and file drafts. This supersedes the earlier shell --split proposal.

Jack Heart selected a distinct Dashboard type (name provisional) that references
existing Task workspaces. Jack explicitly excluded replacing repository/Wave
default views for now. This supersedes default-view binding requirements below:
keep built-in defaults; custom selection and arrangement belong to separate
Dashboards. Preserve Task panes/drafts and saved custom arrangements. The current
design's accepted direction owns the correction; creation/pinning UX remains
unresolved, while current shared command spellings are documented below.


Jack Heart accepted the query-view prototype as good enough to build and
requested a new pursue worker. [design.md](design.md) records that authorization
and the accepted LOO-489 navigation/search contract. The full October 16 draft,
including proposed grammar, commands, acceptance scenarios and source survey,
is preserved at `refs/loopflow/evidence/loo-497-before-compress:desktop-api-coordination.md`
(tree `a0a62123ff32addaf29cc11570b3263a5630fff1`). It is design history, not
additional authority. The accepted product direction remains in
[arrange-navigation-brief.md](arrange-navigation-brief.md) and
[layout-editing-direction.md](layout-editing-direction.md).

## Shared contract now available — October 17

`SqliteStore::view_plan` / `plan_selection` return `ViewPlan`; `selection_plans`
feeds built-in repository/Wave defaults and Dashboard plans into
`PlanningPart.selections`. `edit_dashboard` preserves clause IDs and checks the
saved revision. CLI: `desktop view EXPRESSION --plan`,
`desktop query ID EXPRESSION --revision N` (ID or `query:ID`). Swift
`WorkViewPlan`, `Dashboard` and `RegistryQuery.editDashboardQuery` mirror the wire.
Rust owns evaluation, current attention and first-wins projection.

Dashboard identity (`DashboardId`) is separate from selection text. The existing
schema draft owns `dashboards` and expression-keyed `dashboard_arrangements`.
`set_dashboard_metadata` pins/names Dashboards; `save_dashboard` retains per-expression
arrangements. `pane_layout` / `save_pane_layout` retain Work-owned pane IDs/content,
locations, focus/zoom/hidden and file drafts (path/base revision/base text/edits).
CLI `pin`, `unpin`, `rename`, `arrange` and `layout` expose those operations;
Swift `RegistryQuery` supplies typed callers. All writes compare revisions.
`ViewPlan.dashboard_id` identifies custom plans; it is absent on built-in defaults.
Two Dashboards may independently select the same Task and read the same pane record.
There is no binding/default-replacement command, DTO or storage table.

October 17 tab cut: portable Shell now owns ordered `{id,location,lf_process_id}`
tabs plus `selected`; Files owns `location`, ordered `{id,path}` tabs and optional
`selected`. One tool of each kind per Task is validated atomically. Native
`MultiplexerStore` and `TaskFilesStore` use this model; one Files document store
handles Task aliases. Shell creation adds/selects a tab; file reopen reuses a tab.
`ShellTab.id` is `TerminalIdentity.shell` identity. Inspection includes `tabs` and
`selected_tab`; exact targets carry optional `tab` and its incarnation. Receipts
return tool `pane` plus new `tab`. CLI `--pane shell:ID` addresses background tabs.
Non-Task shell requests mutate nothing; the split-as-shell command is removed.

Native conversion: `MultiplexerStore.paneLayout(work:revision:files:drafts:)` and
`restore`, `TaskFilesStore.paneIntent(location:)`, `drafts(location:)`, `restoreTabs`
and `restoreDrafts`. Cold-restored shells have no command and remain unattached;
recovered documents stay read-only until disk reconciliation. These are the shared
boundary for LOO-489 persistence callers and LOO-416 Process reattachment, not an
application persistence loop or a replacement lifecycle owner. Existing native
Session launch behavior remains outside the tab decision.

The application persistence and live selection integration are still missing.
LOO-489's renderer and LOO-416's attachment changes remain local work without a
combined code handoff. No sibling dirty files were copied. Cold restore creates
fresh native incarnations. Warm conversion reuses matching shell runtime objects
and skips already-dirty documents, but replaces the tree, selection and file-tab
list. Preserving newer tabs/selection during a delayed restore remains application
work. Compare draft bases against current disk and retain
unavailable locations without substituting a local shell. The schema and DTOs
alone do not establish native restoration or cross-machine delivery.

## Remaining contract

- Adopt the implemented portable/native tab conversion boundary in application
  save/restore and lifecycle reattachment. Combine new native state with saved
  records without losing newer shells or drafts. Current tests prove local tab
  identity, selected-tab recovery, draft reconciliation, exact background targeting,
  duplicate-file reuse and non-Task rejection; no process-restart proof is claimed.
- Consume shared selection plans in native WorkModel/navigation; observation,
  current attention ordering and headless `view --plan` are implemented. Preserve
  their passive contract and add explicit native zoom through the coordinated
  pane-intent cut. No Swift membership evaluator or subprocess per tile.
- Live `view/open/query` and UI/deep links converge on one selection entry point.
  Preserve exact Session/Changes intents, opening generations and readiness.
  Work navigation alone must not start a conversation. Repo selection targets
  its own window. Pinning changes discovery; rename uses `work/edit.rs` writers.
- Integrate the retained arrangement contract with one native Task layout per
  Work key. Containers reference Work/query members. Primary Session is the
  default tile face; only explicit zoom overrides it. An absent primary is an
  empty tile, never a fallback to the focused shell.
- Apply first-visible-wins to the whole repository window's displayed composition,
  first child then second, skipping hidden members. Suppressed occurrences retain
  query membership and identify their winner. Focus cannot choose the winner;
  promotion must reuse the same Task layout, Session and surface.
- Full-tree replacement includes every occurrence once, including hidden members,
  and checks the observed revision before changing layout or focus. Natural
  split/move/swap/resize edits transform that tree. Hide A, edit B, restore A
  preserves B's newer edits. Structural edits mark touched; focus/zoom/hide do
  not retile. Incomplete evidence retains intent; complete departures prune.
- Exact occurrence/container targeting resolves repeated Work and ambiguity.
  Input/read fences remain independent. Do not automatically retry mutations
  after a lost Apple-event reply; inspect their effects first.
- Persist portable intent through lfdb; save failures remain visible. Native
  handles, window incarnations, attachment claims and liveness are never saved.
  Independent Dashboards, per-expression arrangement recovery and portable
  Work pane/draft records now exist. LOO-489's native save/restore callers and LOO-416's
  continuity/reattachment remain required. Cross-machine identity mapping and
  transport are unimplemented; local storage does not prove another Machine's
  restoration. Work rename covers Tasks through their existing revisioned writer;
  Wave rename retains `lf wave rename`, with native rename routing still pending.
- Preserve the Task's scripting acceptance: JSON results, target resolution and
  exit statuses (0 success, 1 failure, 2 bad arguments, 3 missing/ambiguous,
  4 unreachable, 130 interrupted), scoped LF_TASK/LF_SESSION/LF_REPO context,
  and send/capture/wait/run-wait where needed. Readiness/events remain LOO-453's
  shared contract; program status remains LOO-422/415's. UI readiness, provider
  readiness and process exit stay separate. This is remaining integration, not
  authority to duplicate those owners or infer attention from idle time.

## Delete — do not maintain during live integration

Removed in the tab cut: `MultiplexerStore.split`, `SessionsWorkspace.split`,
`newShell(beside:focus:)`, pane-keyed shell commands, companion replacement's hidden
shell cache, old portable Shell/Files payloads, split-as-shell CLI/action and
exclusive shell-split fixtures. Tool moves and Session geometry tests retain
arrangement coverage; exact tab targets retain input/read fences.

- Membership cases in `DesktopPaneAction/controlPane`, replaced by selection
  commands while preserving input/text/read authority.
- Leaf-dropping `SessionsWorkspaceRegistry.hide`, replaced by retained membership
  plus visibility. Explicit companion removal/Undo still invalidates occurrences.
- `WorkProjection.inWorkingSet` and affected Swift working-set predicates once
  shared Rust results replace their consumers. Defaults include paused/idle
  started unfinished Tasks and empty child Waves.
- Direct view/deep-link selection mutations superseded by the shared entry point;
  retain stale-opening tests, drafts, exact Session/Changes intents and identities.

The `desktop list` → `state` cut is already in the foundations diff. Do not
replay it; remove the predecessors above as the remaining integration replaces them.

## Current integration ownership

`native-console-integration.md` supplies the materialized 489 source and order of
work. 497 integrates its Dashboard/editor/persistence callers into that Console;
489's ordinary navigation and 416's provider lifetime owners stay intact. The
older unavailable-handoff readback is preserved in the local source snapshot's
`loo497-before-continuation-desktop-api-coordination.md.txt`.

416's transport stack includes merge `7bfa10fa72` / `2523534b29`: `session attach`
(including observation/Machine routing), `transport start` and `transport attach`.
Attach never starts/resumes a provider. Its local metadata-only admission and
attach-if-free path still need a supported source handoff if this integration
consumes them; do not infer cold Process continuity from portable records.

## Integration acceptance

Gate will seed a real disposable repository with started running/paused/idle,
backlog and finished Tasks, primary Sessions, a shell, empty/child Waves, nested
queries and incomplete evidence. Produce plans through the built CLI, decode
in Swift, and exercise production selection/arrangement entry points without
mounting windows. Prove click/CLI membership parity, stable native identity,
focus versus zoom, hidden-edit recovery, duplicate promotion, stale/invalid map
atomicity, departure versus unavailable retention, reload, pin/rename propagation
and no read-time execution. Linux proves headless planning too. Mock external
process/network effects only; use the candidate CLI and a private Machine.

Native rendered continuity belongs to demo and LOO-416. The new pursue Flow
implements the remaining accepted contract and publishes for demo review.
No merge or Task completion is authorized by this launch.

Production save/restore proof must include a saved two-shell/two-file Task,
newer tabs/selection/drafts created while its read is pending, and a stale write
rejected without replacing either live edits or newer durable state. Navigation
between independent Dashboards and built-in defaults must retain the same Task
owner. Cold shells restore without launch argv; only LOO-416's attach path can
establish Process continuity. Direct conversion tests do not satisfy these cases.

Current 489 source is materialized and verified; older statements requiring a
combined commit are superseded. See `native-console-integration.md` for the
precise source versions, current Console contract and accepted query correction.
