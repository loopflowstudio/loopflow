# LOO-903 · Backlog exploration

## Direction to build

Jack Heart, November 19, after answering the Start bar questions ("1 a 2a 3b 4 always have it for built in, not required for customs"): "lets build this!"

The native preview `scratch/native-backlog-preview/BacklogPreview.swift` (build 18) is the reference for everything below; the review pages are `backlog-actions.html`, `backlog-start.html` and `backlog-start-bar.html`.

- **Layout.** The Wave view keeps its conversation on the left and puts the backlog on the right, using production `WaveViewLayout`/`RatioSplit` with the conversation's default share at 0.236. Hide and zoom work as they do today.
- **Header.** Two lines. The first holds the scope, the chapter, the objective and New. The second holds the filter segments "All N · Mine N · Available N" and search.
- **One ranked list.** Each row shows its ID and title, its Wave when that differs from the scope, and an owner mark: filled for you, muted for someone else, a dashed ring for unclaimed.
- **Selection.** The whole row is the click target. Selecting one Task opens its brief in place; a brief over 900 characters is cut at 9 lines with "Show all N words". Several selected show no brief. ⌘-click, ⇧-click, the hover checkbox, ↑↓ and ⇧↑↓ all select.
- **Toolbar under the header.** It acts on the selection and is disabled with none:
  - Start, a split button (detailed below)
  - Claim, with a count when several are selected
  - Edit
  - Move ▾ (To Wave ›, To top)
  - the position, or "N selected"
  - ↑ ↓
  - Abandon…
- **Row menu.** "···" and right-click open the same menu: Start · <flow>, Start with ›, Claim, Edit brief, Move, Move up/down, Copy ID, Abandon….
- **Ownership.** Claim works without starting. Mine and Available filter on it. Every empty state offers its one next step.
- **Ordering and undo.** Drag moves one or several rows; ⌘↑↓ and To top also reorder. ⌘Z undoes reorder, claim and edit. Start and Abandon are not undoable.
- **Keys:**
  - ⌘↩ starts
  - C claims
  - E edits
  - ⌘⌫ abandons
  - Esc clears the selection
  - ⌘A selects all
- **Start picks a flow.** The button reads "Start[ N] · <flow>". Clicking it runs the default; ▾ lists the flows.
  - Each flow shows its description line and a tag with its stop count, or a warning tag for flows that merge or never stop.
  - Flows with no stop sit under "Run without stopping".
  - When the selected Tasks' Waves default to different flows, the button shows the breakdown ("Start 3 · feature 2 · code 1"). The menu's checked first item, "Each Task's own workflow", keeps each Task's own; any flow below it overrides all of them to that one.
- **Graph descriptions.** Graph files gain a `description:` key, required for builtin Graphs and optional for custom ones. The builtin descriptions come from the header comments the files already open with. `lf graph list` shows the description, and Start's menu reads it. The stop count is derived from the `interactive:` steps.
- **Not in this build:**
  - regrouping and nesting rows
  - a command palette
  - the delivery experience after Start (LOO-434 / PR #1851), which Start hands off to

Jack Heart, November 19, on build 18's screenshot: "still looks a little understyled, but lets build the real thing". His steer on it, condensed:
1. Start rendered as a plain grey split button, because a SwiftUI `Menu` with `primaryAction` ignores `.borderedProminent`/`.tint`. Build Start as a real accent-filled split control, a filled main part plus a chevron part. It is the one filled shape in the bar.
2. Claim and Edit got a grey fill while Move did not. Give the secondary actions one consistent quiet style.
3. At the default backlog width the toolbar clipped Abandon off the right edge. The bar must fit or collapse: ↑↓ and the position go first.
4. The All/Mine/Available segmented control drew in system blue. Use the Loopflow palette accent.

Style from the production design system, not the preview.

## Implementation plan

### Demo

Open a Wave in the Loopflow Dev app; the demo seeds a scratch Wave with eight Tasks so no real plan changes. The backlog fills about three quarters of the Wave view to the right of its conversation.

1. Click a row: its brief opens under it. ⌘↓ twice moves it down, and `lf task list --wave demo` shows the new order. ⌘Z restores it.
2. Press C: the owner mark fills, Mine reads 1 and `lf task status` shows the claim.
3. ⇧↓ twice selects three rows and the brief closes. The bar reads "Start 3 · feature". ▾ lists feature (3 stops), code (2 stops), research (1 stop) and, under "Run without stopping", auto (merges without asking).
4. Pick research: three Tasks start on `research`, the rows leave the backlog, and the sidebar shows them working.

### Done when

- A Wave's backlog is one ranked list beside its conversation. Search, the All/Mine/Available filter, the selection, the toolbar, the row menu, the keys, drag, undo and Start with a flow all work as described in "Direction to build" above.
- Reordering in the app changes the rank the CLI reports, and the change syncs to Linear the way `lf task update --rank` already does.
- `lf graph list` prints each Graph's description. Every builtin Graph has one.
- The old standup board is gone: no Working/Autonomous/Done segments, no "None" rows.

### Acceptance at gate (headless)

| Check | Expected |
|---|---|
| `cargo test -p loopflow-execution --lib graph::` | passes. Includes `every_builtin_graph_has_a_description` and the parse of `description:` + `steps:`. |
| `cargo test -p loopflow --test integration dto_fixtures` | `graph_catalog.json` and `roadmap_snapshot.json` round-trip with `description` and `owned_by_you` present. |
| `lf graph list` | `feature  Explore, build, deliver and verify…`. A custom Graph without a description prints its name alone. |
| `scripts/test_desktop.sh --filter BacklogViewTests` | passes. |

`BacklogViewTests` loads a fixture roadmap with six backlog Tasks: one owned by you, one by someone else, four unclaimed, one of those started and stopped at demo with a PR; two Waves whose defaults are feature and code.
- Filter counts read "All 6 · Mine 1 · Available 5"; the started row counts.
- Search "brief-word" leaves the one matching row.
- Selecting rows 1–3 shows no brief and "Start 3 · feature 2 · code 1".
- ⌘↓ on row 1 issues `task update <id> --rank 1` through a stub query.
- Undo issues `--rank 0`.
- Claim on the other person's Task is disabled.
- Mine with nothing claimed shows "Show available".

CI runs the full suites.

### Source of truth

- **Order.** `Task.rank`, per Project. Every write goes through `planning_order::reorder_in` (`rust/loopflow-storage/src/sqlite/planning_order.rs`), which renumbers siblings and queues the Linear `sortOrder` move (`ops/planning_order.rs`). The client never owns an order. It stops using `task_orders`/`desktop.dashboard.move` for this list; Working-Tasks dashboards keep theirs.
- **Ownership.** The ownership log (`TaskOwnership`, `rust/loopflow-model/src/durable.rs`). "Yours" is `is_mine_in` (`rust/loopflow-storage/src/sqlite/task_ownership.rs`), which checks people id, then Git `user.email`, then Machine. Swift does not repeat that rule. Rust projects `RoadmapTask.owned_by_you: bool` from one `You`, which storage defines and `loopflow_execution::store::ownership::you()` builds, since execution is the crate that reads Git and config.
- **Flows.** The `lf graph list --json` catalog (`GraphCatalogEntry`, `rust/loopflow-execution/src/graph/compiled.rs`), plus each Task's `graph_name`/`graph_source`. The Task's graph is its own, else the Wave default, else the repository's.

### Risk findings

- **Graph files are bare YAML lists.** `parse_graph_items` (`rust/loopflow-execution/src/graph/mod.rs`) already accepts a mapping with a `steps:` key, and ignores any other key. A Graph with a description is therefore `description: …` plus `steps: [...]`. Released `lf` reads such a repository file and skips the description, so there is no shared-file break. Custom Graphs stay valid as bare lists. Builtins (`rust/loopflow-definitions/src/builtins/`) move their header comment into the key and delete the comment, so the text lives in one place. A description containing `: ` is not valid YAML unquoted; two were reworded for that.
- **Desktop has no "me".** No Swift identity exists, and `TaskOwner` carries keys only. `owned_by_you` costs one `OwnershipFacts` read per repository per roadmap read, which spawns `git config`. It must be read once per read, never per Task.
- **The served `task.update` carries only `notes`** (`lf/commands/write.rs:131`; Swift `ServedWrite.taskUpdate`). Add `rank: Option<u32>` to both. The CLI already has `--rank`. Rank is set-position-N, clamped. A multi-row move is a sequence of rank sets in final order, which avoids a new batch op. Each set is one write transaction and one Linear move.
- **The catalog lists builtin, repository and Wave Graphs together,** including pieces other Graphs include. `included_by` and `runs` on the catalog entry sort them: pieces and undescribed Graphs go under "More ›", and a Wave's Graph is not offered for a Task (slice 5).
- **Compiled Graphs are expanded.** A catalog entry's `compiled.steps` holds the steps of every Graph it includes, each node's `sources` naming the Graphs it came through (`tests/fixtures/dto/graph_catalog.json`). Stop counts and the merge tag therefore see through `graph:` includes.
- **Stops and the merge warning are derived in Swift** from `compiled.steps`.
  - Stops: the count of `interactive` nodes.
  - "Merges without asking": an `op` node whose label starts with `land` comes before the first interactive node. deliver ends in `cmd: land --wait-and-fix`, so that is true of auto (pursue + deliver) and deliver.
  - Builtin Task Graphs after main's restructuring, read from the files on November 19: feature 3 stops, code 2, research 1, redesign 1; no stop in auto, incident, deep-clean, optimize, queue, task-design, and in the pieces plan, build, deliver, pursue and refresh.
  - The other zero-stop flows read "never stops". Reaching `end` approves Auto-Ship (`task run`'s help, `lf/mod.rs:2188`), so the "Run without stopping" header says so.
- **Search is client-side over the loaded roadmap.** The selection-view read has no text search (`work/selection/query.rs:363`). A Wave's backlog is tens of Tasks, and the roadmap holds every one unless `truncated`. When truncated, the list says "Partial: searching the N loaded" rather than presenting a complete result.
- **Several selected Tasks starting is N `startTask` calls.** Since main's #1960 (`2593c489d7`), Claim, Start, Move and Abandon show in the click and a refusal puts that Task's row back with its own window Notice. The backlog's batch actions loop over those writers, so there is no "Started 2 of 3 · reason" tally; the footer says "Started 3". "Each Task's own workflow" passes no graph. Any named flow passes it as the positional `graph`.
- **Undo** uses SwiftUI's `@Environment(\.undoManager)` and registers inverse operations:
  - reorder → the prior ranks, registered when the rank writes land
  - claim → unassign, registered in the click
  - edit → the prior brief, only while the current brief still equals what was written. Otherwise the undo stops with "Brief changed since; not undone".
  - Start and Abandon register nothing.
- **Repository scope mixes Projects,** and rank is per Project. Reorder, ⌘↑↓ and To top act only in Wave scope. In repository scope they are disabled with "Order lives in each Wave". Search, select, Claim, Start, Move and Abandon work in both scopes.

### Target architecture

- `BacklogView.swift` replaces `WaveStandupBoard.swift`. It takes the same `backlog:` slot of `WaveViewLayout` (`SessionsView.swift:1848`) and `WorkSurfaceView.swift:78`. Its parts:
  - `BacklogList`: an `@Observable` value type holding the selection (`Set<TaskID>`, anchor, cursor), filter, query and showsAll. It is projected from `WorkModel.standupTasks` filtered to `section == .backlog`, in rank order. It also carries filter counts, `selectionFlows`, `canReorder` and the row owner mark.
  - Views: header (scope · chapter · objective disclosure · New), filter + search line, `BacklogToolbar`, rows, brief, empty states.
  - Keys go through one `.onKeyPress` handler on the list. The preview's NSEvent monitor is not used.
- `BacklogToolbar`, shaped by the November 19 steer:
  - **Start** is a custom split control built from two `Button`s in one accent-filled capsule from `DesignSystem`, with a divider. The chevron opens an `NSMenu`-backed `Menu`. It is the only filled shape.
  - Claim, Edit, Move ▾ and Abandon… share one quiet borderless style.
  - `ViewThatFits` drops the position text, then ↑↓, before anything clips.
  - The segmented filter is a custom capsule picker in the palette accent, not `Picker(.segmented)`.
- `WorkModel` gains `rankBacklog`, `claimBacklog`, `startBacklog`, `moveBacklog`, `abandonBacklog`, `saveBacklogBrief` and `revertBacklog` (in `BacklogView.swift`), each a loop over the single-Task writers `rankTask`, `assignTask`, `startTask`, `moveTask`, `abandonTask` and `updateTaskDirective`. Rank writes are awaited in turn and stop at the first refusal; the others show in the click.
- `WaveViewRatios.session` default becomes `(1 - golden) * golden` ≈ 0.236 (`WorkProjection.swift:223`).
- `lf desktop capture -p backlog` prints the backlog text: the filter counts, then the rows in order with owner marks. It comes from `BacklogList.text`.
- Rust:
  - `GraphDefinition.description: Option<String>` and `GraphCatalogEntry.description: Option<String>` (a required nullable wire key).
  - `lf graph list` prints `name  description`.
  - `RoadmapTask.owned_by_you: bool`.
  - Served `task.update` gains `rank`.
  - Fixtures are updated in `tests/fixtures/dto/` and in each language's fixture test.

### Delete — do not maintain

- All of `WaveStandupBoard.swift`'s board-only types: `WaveStandup` (Row/Person/SessionAttention), `WaveStandupDefaults`, `WaveBoardList`, `WaveDoneRange`, `WaveStandupRow`, `WavePersonPicture`.
- `WaveBoardSection` and `navigation.boardSections` (`WorkProjection.swift:215/257`).
- `WorkModel.standup`, `boardSection` and `standupText`.
- The `section` and `filter` Desktop actions (`DesktopActions.swift:310-333`) and the Done-days default.
- `WaveViewHeader` moved into `BacklogView.swift`; `PaneVisibility.swift` still uses it. `WavePlanningNotice` and the backlog's inline "Planning unavailable / Retry" line are deleted: main sends read failures and the out-of-plan notice to the window Notice.
- Tests: `WaveStandupTests.swift`'s projection tests go. Its view tests for hide/zoom/resize (`wavePartsHideOnHover…`, `wavePartsZoomResize…`) are rewritten against `BacklogView`, without `section`. `TaskHistoryFilterTests.waveBoardShowsTheWave…` is updated to match.
- `githubPeople` / `loadGitHubPeople`, Swift `RepoPerson` and the `repo.people` Swift read: deleted, the board was their only reader.
- `PlanBoardTask` (`rust/loopflow/src/lf/commands/screens.rs`) stays: `lf plan board` reads it. Swift `PlanBoard` has only fixture readers.
- The scratch preview `scratch/native-backlog-preview/` is a reference only. Nothing from it is copied as-is. Its NSEvent key monitor, `PreviewSplit` and fake data stay in scratch.

### Forbidden outcomes

- A second order: a client array, `task_orders` for the backlog, or a Swift-side rank.
- Swift deciding who "you" are.
- Keeping the standup board beside the new list, or behind a flag.
- A batch Rust command added only to loop over single writes.
- Hard-coded flow descriptions or stop counts in Swift.

### Exclusions

- Regrouping and nesting rows (LOO-996 territory).
- A command palette.
- Post-Start delivery (LOO-434 / #1851).
- Reopening abandoned Tasks.
- Server-side text search.

### Slices

Status, November 19: slices 1–6 are built (6 at `fc22206d35`). A third concept review of that build found nothing to change: the row menu, the button, ⌘↩ and the docs agree, and Open (`openTaskDestination`) runs no Graph. The branch holds main through `f76129f677` (merge `09c29fd429`, #1991), which touched no backlog, Graph or Desktop file. Nothing has been seen in the real app.

1. **Rust — built.** `description:` on Graph files, `lf graph list` prints it, catalog DTO in both languages, `RoadmapTask.owned_by_you`, served `task.update` takes `notes` and `rank`.
2. **Swift list — built.** `BacklogView.swift`; `BacklogState` lives in `WorkNavigation.backlogs`; `BacklogList` is the projection. Default conversation share is 0.236.
3. **Actions — built.** Toolbar, row menu, keys, drag, rank reorder, Claim, Edit, Move, Abandon, undo, a footer line per action and the window Notice for refusals.
4. **Capture text and docs — built.** `lf desktop capture -p backlog` prints counts then rows.
5. **Concept review's seven changes — built** (the review's findings are in git at `0fe4d9afef`):
   - A started Task nobody holds says where it stands ("at demo · PR #12", from `execution.step`, else the last Graph step, else `next_move.reason`) in the row and the capture text, and has no workflow menu. Start is off, with Rust's reason, when `run_control` refuses every selected unstarted Task. Slice 6 replaced its Continue button.
   - `BacklogList.selection` is the stored selection cut to the shown rows, and every action reads it; a changed filter or search also drops the hidden rows from the stored selection (`BacklogState.keep`). The Abandon dialog lists identifiers (five, then "and N more") and names the Tasks with a PR.
   - `NewTaskSheet` has File: `WorkModel.fileTask` runs `lf task create` without `--run`, waits for the answer (a refusal stays in the sheet with the words), clears that backlog's filter and search and selects the new row.
   - `GraphCatalogEntry` carries `included_by: [String]` and `runs: task | wave` (builtin category; repository Graphs are `task`). `StartFlow`/`StartFlows` is the one layout: stopping, More (pieces and undescribed), "Run without stopping"; Wave Graphs are left out. Start's menu, `WorkflowChooser` and the Task view's "Choose workflow" all read it (`StartFlowItems`).
   - Desktop strings and `docs/surfaces.md` say "workflow".
   - "Each Task's own workflow"; `?? "code"` and `defaultFlow` are gone, and a Task with no `graph_name` is started without a name; Start with nothing selected reads "Start", disabled.
   - The view part is `backlog` (`WaveViewPart.backlog`, `DesktopViewPartKind::Backlog`, `-p backlog`, `BacklogView`, `BacklogViewTests`). `WorkNavigation.Board` stays.

6. **Second concept review, November 19 — built.** A started row opens; it does not Continue. `lf task run` from a Task's last stop reaches `end` and approves Auto-Ship (`lf/mod.rs:2188`), so Continue on the fixture's "at demo · PR #12" row would ship a PR someone else left; "What was observed" already says Backlog Start "must not reproduce Continue/Ship semantics here" (LOO-434 owns that).
   - One started Task selected alone: the button reads **Open** and goes to the Task (`BacklogList.opens`, `WorkModel.openBacklog` → `openTaskDestination`). Open is the first item of every row's menu, off for several rows.
   - Start, ⌘↩ and "Start with" reach unstarted Tasks only: `startBacklog` filters to `awaitsStart`, and `perform(.start)` returns when `startUnavailable` is set, so the key and the button share one predicate. Both kinds selected: "Start 2 · feature", tooltip "1 started Task: open it to continue". Several started and nothing else: Start off, "2 started Tasks: open each to continue".
   - Claim's footer: "Claimed LOO-781 · now in Working" for a started Task; "Claimed 3 · 1 now in Working" for several.
   - Empty title: "Nothing is waiting in this Wave" / "…this repository".
   - Gone: `startLabel`'s Continue branch, "Start 2 · feature · Continue 1", "Continued N", the docs paragraph "A started Task keeps its Graph…".
   - Kept: the row's standing, the Abandon dialog naming PRs, no workflow menu on a started row. `runs` from the builtin directory still offers a repository's Wave Graph for a Task (in `scratch/questions.md`).

Remaining:
- The demo and the hands-on run in the Dev app: drag feel, key focus, the split Start's menu rendering, toolbar collapse at narrow widths, the File button beside Start, the chooser's new rows. The seed is a scratch Wave with eight Tasks in a private Machine, built the way `.lf/skills/demo.md` and `.lf/qa/desktop.md` describe. Backlog rows have no `lf desktop` socket route: `lf desktop capture -p backlog` reads the list, and selecting, dragging and the menus need a person or headless view tests.
- Reorder has no optimistic projection: rows move when the Work update arrives, while Claim, Start, Move and Abandon show in the click.
- `lf graph show NAME --json` now compiles the whole catalog to fill `included_by`. Not measured.

Left as built: the "merges without asking" tag comes from a Swift match on step labels beginning `land`/`pr land`; incident has no such step and reads "never stops", though reaching the end approves Auto-Ship there too. The tags were shown in the Start bar page and answered "2a".

Checks (headless, this Machine):
- After the #1991 merge, with item 6: `scripts/test_desktop.sh --filter 'BacklogViewTests|NewTaskTests|DTOFixtureTests|TaskHistoryFilterTests|DesktopHeadlessTests|WorkModelTests|TaskGraphProofTests|SessionChromeProofTests|TaskConceptTests|WorkDestinationTests'`: passed (display-session suites skip). `cargo nextest run -p loopflow -p loopflow-execution -p loopflow-definitions -E 'test(graph::compiled::) | test(dto_fixtures) | test(graph_discovery_tests::) | test(documented_commands::) | test(lf::tests::) | test(server::schema::tests::) | test(view_part)'`: 116 passed.
- Not run since the sync: `cargo clippy --all-targets -- -D warnings`; `served_reads::screen_reads_are_compact_passive_and_match_cli`, which failed identically on base 0a31dd3c98; the iOS suite, which shared Swift models reach. Gate and CI run them.
- Seen in the sync's full `loopflow-execution store::` run: `task_labels_and_generated_names_follow_local_identity_and_linear_mapping` failed and passed alone. Not diagnosed and nothing filed.

Exploration history follows. Explore round 6, November 14, 2026, combined the composed reading workspace, compact rows and a separate full-width repository conversation. Review page: [explore.html](explore.html).

## What was observed

Installed Loopflow Desktop, November 14, 2026, approximately 17:00 PST:
- Repository sidebar showed 11 working/autonomous Tasks. Several long titles and summary lines were visibly truncated. Program reports such as Done appeared alongside Tasks still in the working set; a program report is not Task completion.
- Product showed Backlog 48, Working 7, Autonomous 1, Done 0. The backlog displayed IDs and titles, including LOO-452, LOO-514, LOO-590 and LOO-794. No search, selection for batch operations, brief excerpt or ordering control appeared in that board.
- Product's conversation reported “Session failed to open” and “The recorded checkout is unavailable; it has not been recreated”. The planning board remained readable. The sidebar simultaneously said “No working Tasks in this Wave”; the count/row disagreement was observed, not diagnosed.
- Navigation changed while inspection was underway; the UI tool twice rejected input because “The user changed '/Applications/Loopflow.app'”. No Task was started, edited or reorganized during inspection. Reading an unstarted Task's native destination and the repository planning board remain unobserved.
- Shared `task show LOO-452` returned its complete brief, unstarted state, Product membership, Project rank 6 and no worktree. Its brief separates customization from LOO-403's discoverability, and also covers programmatic terminal chords. A title alone hides that distinction.
- `task status LOO-514` showed unstarted, no Sessions, no Graph runs and no PR. Its relationship between Wave backlog/conversation/dashboard remains its own design work.
- LOO-434 / draft #1851 contains three delivery prototypes: route, next decision and past delivery. Its page source and design were inspected. No direction is chosen there. Backlog Start should hand off to whichever shared delivery experience is selected; it must not reproduce Continue/Ship semantics here.

The prototype includes eight abridged, real Task titles with illustrative brief excerpts, membership and state labels. It is a local simulation, not a copy of the complete backlog or a native acceptance result.

## Existing mechanisms

`WaveStandupBoard.swift` is shared by repository and Wave, sorts by existing rank, projects Rust TaskSection and opens a Task destination on row selection. It pages visible rows. It currently has no search or batch controls; empty sections say “None”.

`TaskStartOffer` shows up to eight brief lines beside explicit Start. Description editing exists in `WorkSurfaceView.swift`. `RegistryQuery.updateTaskDirective` reaches the shared writer. CLI `task update` also supports title/rank/assignee; the Swift operation currently exposes notes only. Membership moves exist in both surfaces. NewTaskSheet's primary action is Start and calls create-and-run. Plain CLI creation already files an unstarted Task without running it.

TaskSection is the board classification, not a progress score. Backlog can include started unassigned work. Every design must show the row's actual execution and owner separately and use shared legal actions. A filter called “Not started” must check state, not merely section.

## Round 9 · The reader without a side pane

Jack Heart, November 19: “Lets go back to iterating on the "roudn 6 version though" - that side pane is definitely not right. maybe we want expand ?”

[explore.html](explore.html) keeps round 6's compact rows, context line, reorder and separate conversation, and tries four ways to open a brief on all 46 full briefs: expand in place (several at once), one card at a time, take the page (Esc returns to the row), and summary then all. No choice is recorded. The side-by-side reader is withdrawn. The chosen one goes into the native preview next.

Jack Heart, November 19, on what the simple build keeps open for grouping: “Multi-select: definitely keep for multi start/claim. not sure that the header rows will want to look like the main rows but rows that can nest seems like a reasonable place to start”

The page gained row checkboxes with Start and Claim previews for the selection. Group headings are not drawn; how they look is open.

Jack Heart, November 19, on the four ways to open a brief: “a probably”

Expand in place went into the native preview: `BacklogPreview.swift` now has one full-width list, rows that open under their title (several at once), row checkboxes with Start and Claim previews, and Edit, Start and Move inside an opened brief. The side reader and its uppercase column labels are deleted. Swift 6 build passed and the review bundle was relaunched (PID 33353). A window capture taken seconds after launch showed a row being dragged by hand with an insertion marker, a changed order (LOO-456 first) and “Order updated in preview only”, so pointer drag does reorder. Opened briefs, long-brief height and multi-select were not exercised by me in the native app.

Jack Heart, November 19, on the native expand-in-place preview: “looks pretty decent”

Jack Heart, November 19: “what about multiselect, and where should the actions (abandon/start) etc go”

Jack Heart, next: “and we dont any of this now but were going to need:\n- claiming without starting / claiming\n- filter to claimed\n- filter out claimed by others ("available"?)\n- buttson for the regrouping”

Tried in the native preview (first bundle, `Loopflow Backlog Preview.app`): click opens a brief; ⌘-click and ⇧-click select, with a checkbox that shows on hover or once anything is selected; one action set (Start, Claim, Move to Wave, Move to top, Abandon) reached from a floating selection bar, a row's context menu and a “…” in an opened brief; Edit stays in the brief. Claim works locally without starting; a filter offers All, Mine and Available. Owners are invented (three Tasks marked as someone else's). Regrouping buttons are not drawn. None of this is chosen.

Jack Heart, November 19, on the second preview: “clicking seems to be gated on the title rather than the whole row”, then “also weird that multiple can be expanded and not selected?”

Changed in the preview: the whole row is the click target. Opening a brief now selects it and closing deselects, so every open brief is in the selection; ⌘-click, ⇧-click and the checkbox still select without opening. Start and the “…” menu left the opened brief, which keeps Edit and Move; the selection bar and the context menu are the two places actions live.

Jack Heart, November 19: “i think basically selected = expanded, multi selected = no expansion”

Jack Heart, next: “what is prior art for this kidn of design”, then “can you run a comparison for what weve built against some of the best UIs in this space”

The preview now has one state, the selection. One selected Task shows its brief, with Edit, Start, Claim and a “…” menu inside it. Several selected show no brief, and a floating bar carries the same actions. Several briefs open at once is gone.

Prior art and comparison: [backlog-comparison.html](backlog-comparison.html), read from documentation on November 19, not from using the apps. OmniFocus 4 has the same model (a selected row expands; several selected go to the Inspector). Things 3 expands in place but opens on Return, not on select. Linear never expands; it peeks on Space and has the bulk model the preview copies (bottom action bar, right-click, Esc). Gaps found in the preview: no keyboard selection, no undo, no multi-row drag, and no reference expands text as long as a brief on a single click.

Jack Heart, November 19, after the comparison: "lets bring the preview up to snuff"

Changed in the preview after that (choices made here, none confirmed):

- Keys: ↑↓ select, ⇧↑↓ extend from the anchor, ⌘↑↓ reorder the selection, Return shows the whole brief, ⌘Return Start…, ⌘⌫ Abandon…, ⌘A, ⌘Z, Esc clears. Arrows work from the search field.
- "selected = expanded" is kept for arrows too. The opened brief shows nine lines with "Show all N words", so arrowing moves a bounded peek and the list does not jump by a full brief. Opening on Return, as Things does, was not taken.
- Undo for reorder, claim and brief edits, by ⌘Z or an Undo link in the footer. Start and Abandon stay preview-only.
- Dragging a selected row moves the whole selection; Move up, down and to top do the same. ⇧-click now replaces the selection with the range.
- Not built: command palette, regrouping buttons.
- Compiled and launched November 19; window capture failed, so keys, drag and render are not observed.

Jack Heart, November 19: "your previous comparison was a ltilte specific about the selection state. can you show me this sideby side against linear, notion, orchestrator. do t3/paseo have any backlog management?"

Written: `scratch/backlog-side-by-side.html`, from docs and READMEs, apps not used. "Orchestrator" read as Composio's Agent Orchestrator, with Zac Healy's macOS Orchestrator noted as the other candidate. T3 Code and Paseo have no backlog: T3 has in-thread queues, schedules, snooze and Settle; Paseo has none.

Jack Heart, November 19: "has theo posted online about WHY no backlog?"

Searched November 19 and found no direct statement. Nearest, from secondhand summaries: a September 9, 2026 episode where he treats the T3 Code sidebar as a to-do list ("I start too many threads and I get lost sometimes") and has an agent sort his open PRs; a November 5, 2026 write-up of a live session in which humans keep "direction, taste and scope" and "The agent shouldn't bother you until the code is ready to merge." A Backlog proposal exists only in a fork (nhorto/t3code issue #1).

Jack Heart, November 19, on a screenshot of the Mine filter with nothing claimed: "ok, this screen doesnt really work?"
Then: "i also think we should be reusing the layout mechanics that we have for the backlog + conversation, but just making backlog bigger by default"
Then, with a crop of the row's right edge ("360 words", "someone else"): "also can we explore 4-5 different visual/laouy redesign for the action menus/buttons here. Id like to see screenshots of the most equivalent actions in notion and linear." and "and maybe 1 ADE of your choice"

Changed in the preview after that (choices made here):
- The filter shows counts (All 46 · Mine 0 · Available 43) instead of a filtered "0 unstarted". Every empty list says why and offers its one step: Mine → "Show available", search → "Clear search" / "Search all", another Wave → "Open Product".
- The header splits into two lines so the filter and search no longer clip.
- The library sits in the Wave view's layout: conversation left, backlog right, with a copy of `RatioSplit`'s dragging divider and hover zoom/hide like `HideablePart`. Default conversation share 0.236 (0.382 × 0.382, i.e. (1 − 0.618) × 0.618) against the Wave view's 0.382. The "Repository conversation" button is gone.
- Production reference: `WaveViewLayout` / `RatioSplit` in swift/LoopflowMac/Views/PaneVisibility.swift, `WaveViewRatios` in WorkProjection.swift (session = 1 − golden).

Action layouts: `scratch/backlog-actions.html`. A Linear menus + bulk bar, B Notion properties, C hover controls, D Conductor-style Start composer, E one selection toolbar. Reference images in `scratch/backlog-actions-ref/` (Linear changelog, Notion help, a frame of Conductor's 0.66.0 video). No image of Notion's bulk-edit bar was found; the public Notion template is read-only and shows no row actions.

Jack Heart, November 19, on `scratch/backlog-actions.html`: "i think i like A + E", then "3a"

Built in the preview: E's toolbar under the header (Start…, Claim, Edit, Move ▾, position, ↑↓, Abandon…), acting on the selection with counts for several and disabled with none. A's "···" menu on every row, with the same menu on right-click: Start…, Claim, Edit brief, Move, Move up/down, Copy ID, Abandon…. Owner marks at the row's right edge (3a): filled for you, muted for someone else, a dashed ring for unclaimed. Removed: the floating selection capsule and the opened brief's button row. A's bottom bar was not added, since E's toolbar already carries several. Keys C (claim) and E (edit) were added. The menus show no shortcut hints, because per-row SwiftUI shortcuts would register once per row.

Jack Heart, November 19, after A + E was built: "looks good. one thing is that we need to now design the start action to have ways to pick a flow. again find some comps to compare against and explore multiple options"

Page: `scratch/backlog-start.html`, references in `scratch/backlog-start-ref/` (GitHub's Assign Copilot dialog with its custom-agent picker, GitHub Actions' Run workflow, Linear's agent defaults, triage rule and @-agent picker, Conductor's composer). Flows taken from `lf graph list` on November 19, 2026; Product's default is `feature` (.lf/wave/product/config.yaml). Options: A a split button naming the default flow, B a start sheet (flow, agent, start step, note), C the flow as a Task property set while shaping, D outcome-named Start menu, E pick where it stops (overlaps LOO-434 / PR #1851). No reference shows where a run will stop or whether it will merge; every option does.

Jack Heart, November 19, on `scratch/backlog-start.html`: "I like A. Now sow me styling up the start bar", then while it was being drawn: "we can defintiely add descriptions to all our built in graphs.", "Also, I think the default should be potentailly a mixture of the tasks are in different waves", "but you should be able to override that to one"

Page: `scratch/backlog-start-bar.html`. Five styles of A's split Start: a native, b capsule with the flow as a tinted tail, c trailing primary (selection summary left, Start right), d two-line Start saying where it stops, e filled Start plus outlined flow picker with group separators. Each shows nothing selected, one Task and three Tasks across two Waves, plus the open menu. Mixed selection: the button says "Start 3 · mixed", the menu's checked top item is "Each Wave's flow" with the per-Task breakdown, and every flow under it overrides all of them. The builtin Graphs in `rust/loopflow/src/builtins/task/graph/` already open with a header comment that reads as a description; the page drafts shorter menu lines and derives the stop count from `interactive:` entries. Wave defaults read from `.lf/wave/*/config.yaml`: Product and Growth `feature`, Infrastructure `code`.

Jack Heart, November 19, on `scratch/backlog-start-bar.html`: "i guess A is fine"

Built in the preview: the toolbar's Start is a split button, "Start[ N] · feature" or "· mixed" when the selected Tasks' Waves default to different flows. Its menu holds a checked "Each Wave's flow" with the per-flow count (mixed only), then each flow with its stop count and menu line, "Run without stopping" for auto and incident, and a link to the Wave's default. The row menu has "Start · <flow>" and "Start with ›". LOO-753 (Infrastructure, code) and LOO-423 (Growth, feature) are illustratively placed in other Waves and show their Wave on the row. Questions 2–4 on the page are unanswered.

## Round 8 · Grouping is later; build the simple version

Jack Heart, November 19: “i think its worth exploring though, and i can see wanting some way to sort of group backlog tasks together, and then launch some sort of agent to consider them all, come up with some sort of holistic plan/design, and manage them all together”

Jack Heart, while a page for that was being drafted: “so i think what i want this task to build is just the simple version that sets us up to expand to that kind of grouping later on”

Jack Heart, next: “sorry, i meant options #2 and 4 are kind of like that”

No grouping page was published; [explore.html](explore.html) is still the six-options page from round 7. Which build “the simple version” names is not yet confirmed in his words.

Kept for the later grouping work, from the unpublished draft:
- Flow: gather Tasks, one agent reads them all and returns one plan (shared decisions, order and stacking, brief edits, outside dependencies), then that conversation conducts them.
- What a group could be: a lead Task filed to hold them; one of the gathered Tasks leading; a small Wave; a planning pass that leaves no group; groups proposed by the Wave's conversation.
- LOO-996's plan sets `parent_task_id` only at filing, so gathering existing Tasks under a parent needs a move-under operation it does not build. It names a Wave conversation conducting its Project's Tasks as “not built here”.
- A real example: LOO-590, LOO-591, LOO-794 and LOO-797 already cite each other (“Consume those slices”; two briefs each asking whether their surface is a browser tab).

## Round 7 · Six fresh options

Jack Heart, November 18, 2026: “can you take a fresh pass at giving me 6 different options”

[explore.html](explore.html) now shows six visions that differ by what makes a few Tasks stand out, not by layout. The round 6 page is kept as [explore-round6.html](explore-round6.html). No choice is recorded.

1. The line: one cut in a Project's rank order; a handful above, a searched pile below.
2. Loose ends: grouped by the Task each brief says it follows. In the November 14 snapshot 26 of 46 name one, across 13 origins.
3. The plan is a file: `lf plan edit`, reorder and verbs as text, with a preview of every change.
4. What the chapter needs: Tasks under the KR they move. No Task-to-KR link was found in the store.
5. Tonight's run: small work marked unattended and launched in batches with Auto-Ship.
6. The backlog that prepares itself (radical): a scheduled agent pass turns the backlog into cited proposals.

Sketches use the 46 real titles and brief openings. Vision 4's placements and vision 6's findings are invented and labeled so. Check: a Node DOM-stub run drove each sketch (line resize, thread sort, plan edit preview, run selection, proposals); not a browser rendering.

Jack Heart, November 19, on round 7: “5 is kidn of intersesting, but goes with the subwave stuff ive been exploring recnetly (see open tasks)”

Read afterwards: LOO-996 (started, checkout `loopflow.subwaves2`) gives a Task a parent; the conversation that filed children keeps conducting them and hears when one stops or lands. Its brief lists chapters and KRs under later deletion. Bearing on round 7: vision 5's batch launch and morning summary are what a parent conversation does for its children; vision 2's “follows” link is LOO-996's `Task.parent`; vision 4 builds on KRs that brief plans to remove. LOO-330 (unstarted) is the older subwave and memory-addressing design.

## Round 6 · Reorder and reclaim header space

Jack Heart: “they should be reorderable”

Jack Heart, with a crop of the Product/objective/chapter header: “[Image #1] this seems like a bad use of space”

Make the backlog rows draggable within their existing Project order. Show a handle and insertion marker, then commit the move on release; selection and brief drafts stay attached to their Tasks. Filtered reordering anchors the moved Task before/after the target in the full order, retaining hidden items. Provide equivalent Move up/down actions. The prototype changes its private array; production uses the existing shared rank operation and per-Project scope, never a client-owned order.

Collapse the large heading/objective/chapter block to one compact context line. Keep Product and chapter visible, show the objective only on disclosure, and retain the repository-conversation action. Give the reclaimed height to the list and reader.

A revised native bundle is built alongside the first one so the first preview's local edits remain available. Both runtimes stay while their processes live. This is still native design exploration, with no production planning writes.

## Round 5 · Direct reader, native context preview

Jack Heart: “i dont think we need backward for ward at all. can i see it in context in the Loopflow Ui”

Remove brief Back/Forward and its history. Selecting a Task replaces the single brief reader; per-Task drafts and collection context remain. The composed library, compact rows and separate full-width repository conversation remain the direction.

Prepare a native design preview under `scratch/native-backlog-preview/`. It directly compiles Loopflow's existing typography, theme, palette and font registration sources, and reconstructs the sidebar/titlebar context around the proposed library. It is a separate preview app, not the installed Loopflow binary or a production integration. A fresh shared `task list` read returned 46 unstarted Product Tasks; the private snapshot provides their full briefs. Sidebar working-Task shortcuts are illustrative navigation, not live status. No provider or planning writer is connected.

The preview is for judging layout, selection, reading, local brief editing and returning from a conversation-shaped destination. It establishes no real provider continuity or live save behavior. No production code or installed app is changed.

## Round 4 · Earlier reader history exploration

Jack Heart: “1a”

Use one brief reader with Back/Forward history. Remove the inspected-brief tabs. History records Task references, not copies of their contents. Visiting a new brief after Back replaces the forward path; opening the current brief again adds no duplicate history entry. History changes only the reader: scope, filters, collection position and per-Task drafts stay retained. Conversation navigation does not enter brief history or launch work.

The composed library, compact rows and separate full-width repository conversation remain the direction. This reply settles the reader interaction; it does not request implementation or Graph advancement.

## Round 3 · Earlier combined exploration

Jack Heart: “Reply in the conversation: 1c, 2a, 3a. Add what to combine or explore further. No design decision is recorded by this page.”

The reply maps to composed reading workspace (1c), compact rows (2a), and separate full-width repository conversation (3a). The trailing page-generated sentence is recorded verbatim; the selection was delivered here in the conversation.

Combine these as a library and substantial brief reader that retain their context. The repository conversation occupies its own full-width destination. The reading workspace therefore composes the collection and briefs; it does not keep the repository terminal alongside them. Returning from either a repository or Task conversation restores the planning scope, filters, selected Task, reading position and drafts. A working Task shortcut still opens that Task's existing conversation directly.

The next sketch has one compact representation and one composed planning workspace. It removes the alternative list modes and companion conversation from the live example. A short set of inspected-brief tabs is a proposed refinement to test, not implied by the three answer codes. They retain Task references and drafts, never another copy of the plan or an execution object. Tabs should have titles, be closable, and keep Task identity visible. Reading a related Task outside the active filter must explain its membership rather than silently changing the collection.

Current review page: [explore.html](explore.html). No “build this” instruction has been given; implementation and Graph advancement remain for the next request.

## Round 2 · Earlier placement and representation exploration

Jack Heart: “A is defintiely right. nOw look at possible inspirations like Notion, Linear, Asana, for other layouts and representations. focus on both the list it self as well as how we navigate to that / how it is contextualized in the experience of hte app. for exmaple, should it take priority over the te repo session?”

Jack Heart: “find other more novel references as well. Obsidian?”

The next exploration keeps A's browsable library as its basis. It varies two independent things: list representation and navigation within the application. No implementation or shipping instruction follows from this exploration.

[Research and sources](backlog-inspirations.md) cover Notion, Linear, Asana, Obsidian, Scrivener, Heptabase, Things and historical DEVONthink docs. Their documented behavior and proposed Loopflow transfers are separated. The strongest novel candidates are Scrivener's simultaneous synopses and Obsidian's retained reading arrangements; neither requires making a graph the backlog.

Three working application sketches on the same page:
1. Repository entry opens the library. The repository conversation is a named secondary destination. Working Tasks still open directly to their own conversations. Compact rows plus a reader are the starting representation.
2. Plan and Conversation are equal surfaces in one repository. Repository entry restores the last surface. Excerpts and inline brief expansion are the starting representation.
3. A pinned library, inspected brief and optional repository conversation share the current workspace. A short reading trail retains related Tasks. This is a composition proposal for existing owners, not another planning database or canvas. Broader Wave arrangements remain with LOO-514.

Every sketch can change its representation among compact rows, excerpts, cards and reading trail. This does not commit to shipping four modes. Search, scope changes, brief inspection, draft edits, conversation navigation and return to planning work on local sample data. The selected list style does not determine navigation priority.

Starting recommendation for review: deliberate repository entry opens planning; working Task entry opens its existing conversation; restoring a window keeps its current destination. This gives the library room without forcing planning over ongoing Task work. Whether the repository conversation should be a separate surface or an optional companion remains open.

## Round 1 · Earlier alternatives

These describe the first exploration; the current round develops A.

### A · A library you can read

Browse the whole scope; expand a brief within the list without losing neighboring titles. Search title and brief, filter by actual state, and switch repository/Wave without moving membership. Capture uses “Save idea”; execution has a distinct reviewed handoff. Editing, rank and batch Wave moves remain in context. Working/Autonomous/Done are the same existing classifications, each with its own execution detail.

Appeal: arbitrary access and context, especially when the next question is unknown. Cost: moderate UI work, shared search/projection, revision-aware draft retention and exposing existing update/move APIs. Risk: too many controls can turn reading into database administration. Keep actions on the selected items. Project order is meaningful only inside its Project: repository aggregation groups by Wave/Project, never invents a global priority. Standalone Tasks have no Project order to silently borrow.

Key prototype interaction: search, open/edit a brief inline, select several Tasks and preview a Wave move; order actions only in Product scope. Capture saves an unstarted sample. Start opens a consequence preview only.

### B · One useful pass

Choose the scope and work through its existing order one complete brief at a time. Keep, move lower in this Project, edit or remove with a reason; a visible pass ledger offers Undo without losing position. A pass ends with a smaller set worth considering, rather than a new workflow status. Jump back to any item. Capture appends to the backlog, outside the current pass.

Appeal: makes a neglected backlog approachable and gives each brief enough attention. Cost: moderate, including transient pass/undo state, safe rank changes and retained drafts; removal must use shared abandon/reopen operations and show per-item failures. Risk: serial review can obscure cross-item relationships and pressure arbitrary disposal. Keep “leave unchanged” cheap. No mandatory freshness ceremony, inbox state, new later bucket, scheduled deferral or agent worker.

Prototype: leave unchanged, move lower and undo; inspect/change brief; complete a pass and inspect its ledger. The sample pass freezes the initial IDs and rank updates do not reinsert an item into the same pass. “Lower” is deliberately restricted to Product's Project, not cross-Project ordering.

### C · Start with the outcome

Write the result you want next. The planning surface brings existing Tasks and their actual briefs into a temporary comparison, explains relationships, and drafts the smallest useful scope. Read and alter the proposed words before choosing an existing Task or saving a new unstarted idea. Execution is a later separate action. In radical success, a large backlog becomes useful material for shaping work rather than a pile to maintain.

Appeal: the backlog helps compose a better next action, including discovering existing work before duplicating it. Cost: highest—shared content search, source-linked suggestions, draft comparisons and eventually agent assistance. Risk: convincing generated relationships can erase distinct commitments or make the system feel autonomous. Every suggestion carries source IDs; original briefs remain available, suggestions never write, claim or run anything. Scripted matches in this prototype are labeled as such, not represented as live inference.

Prototype: choose a planning question, compare linked real Task excerpts, change the proposed brief, then preview saving it to one existing Task. A merge-of-Tasks proposal is intentionally not enacted: identity/history, source disposition and partial failure need a shared operation before consolidation can be a single action. Reading related started work offers a conversation destination, not another Start. LOO-514 remains separate in the comparison.

## Common contract and implementation constraints

- All three retain existing Task IDs, membership/history and execution owners. No second planning database. Reading or editing never prepares a checkout, launches a provider, claims work or advances a Graph.
- Save idea is plain creation with an explicit destination. Start uses shared run controls, including ownership and live-run checks. The page only previews that handoff. LOO-906 owns immediate conversation presentation for create-and-start; reuse it. Main's #1964 (`b0d867620d`, LOO-932) already starts a new Task straight into its conversation before the checkout exists, and `NewTaskSheet` uses it.
- Drafts and scroll/selection survive navigation, failed saves and external updates. Report stale or conflicting revisions and let the person reconcile their words; never silently overwrite a changed brief.
- Loading retains names/last-good rows, unavailable keeps drafts and explains retry, empty offers capture, no matches offers clear search. LOO-905 owns broader loading/name retention repair; consume it.
- Batch actions expose exact Tasks and effects before writing; show success/failure per item. No implied atomicity across existing single-Task APIs. A failure never invites replay of completed items. Undo must account for newer edits rather than restoring a stale whole record.
- Defer initially means lower rank in the same Project, with no promised date or new lifecycle state. A scheduled return is an open concept, not assumed.
- Remove means abandon with existing identity/history retained and reopen available; permanent deletion is excluded from the opening proposal. Consolidation requires source-linked review and deliberate disposition, not inferred duplicates or automatic closure.
- CLI and agent actions use the same update, move, create and run meanings. Any new multi-Task or consolidation operation belongs in Rust before Swift consumes it. No client-owned ranking score.
- Search/pagination must be shared and bounded, not filtering the first loaded page while presenting a complete result. Query against realistic ~10,000-Task stores; retain reading freshness and revision rules. Do not add transcript-triggered backlog refreshes.
- Delete replaced board/brief/capture paths in the chosen cut. Do not add a permanent parallel planner. This exploration adds no production paths.

## Review threads · as of the exploration rounds (November 14)

The latest correction removes reader history entirely. The current direction has one direct reader and retains drafts by Task. No layout question remains pending from these rounds.

Capture's Save idea versus Start distinction remains. Ordering, batch organization, failure recovery and realistic native use remain required; the focused navigation sketch does not establish them as complete.

## Exploration checks (November 14; production code was unchanged then)

Production Rust/Swift are unchanged. Prototype controls operate only on in-memory sample data; reload resets them. Native backlog editing, large-store search, provider handoff, failures across real writes and retention during running work remain implementation/demo checks. Browser preview tools reported no connected browsers and “Browser is not available: iab”; rendered interaction validation remains unverified unless a later check records it here.

- `node /tmp/loo-903-prototype-check/check.cjs scratch/explore.html` passed on November 14: DOM simulation of initial rendering, search/empty results, draft retention across scope changes, sample reorder/Start preview/batch moves/capture, pass edits/undo/abandon/completion, source comparisons, draft preview, failure recovery and reply selection. No duplicate IDs or network assets. This is not a browser or native rendering result.
- Script syntax and `git diff --check` passed. The pass navigation was corrected so editing alone does not count an item as reviewed or skip it.

## Round 2 checks

- `node /tmp/loo-903-prototype-check/round2.cjs scratch/explore.html` passed: three navigation models and restore rules, four list representations, scope/search/empty results, sample capture, correct Task handoff identity, brief and conversation draft retention, reading trail and companion controls. DOM simulation, not native or browser rendering.
- Reference research and the review page were updated in this checkout. Production code remains unchanged; LOO-903 remains at explore with no Graph process or PR.

## Round 3 checks

- `node /tmp/loo-903-prototype-check/round3.cjs scratch/explore.html` passed: compact library/reader, full-width conversation, scope/filter retention, related-brief tabs/close, retained brief/repository drafts, distinct Task identities and reply drafts, empty/capture, passive Start preview and reset. DOM simulation only.
- `git diff --check` passed. Browser rendering and native retention remain unverified.

## Round 4 checks

- `node /tmp/loo-903-prototype-check/round4.cjs scratch/explore.html` passed: Back/Forward, forward-path replacement, duplicate-visit suppression, retained brief drafts, collection scope/filter retention, conversation return and capture history. DOM simulation only; native scroll/input retention is unverified.
- `git diff --check` passed. Task remains at explore; no implementation or Graph was launched.

## Round 5 checks and native presentation

- Direct-reader DOM check passed: no history controls; per-Task draft retained across selection and conversation navigation.
- Native preview compiled with Swift 6 and production design sources, then opened through the desktop UI tool. Accessibility and screenshot confirmed a 46-Task Product list and full brief beside the Loopflow-style sidebar/titlebar.
- The app remains open for review. Runtime identity and teardown are in [native preview notes](native-backlog-preview/demo.md). This is a separate native design prototype, not the production app or a live provider demonstration.

## Round 6 native checks

- Swift 6 native build passed; compact header and its objective disclosure were inspected in the opened preview.
- Move up/down uses the same local reorder operation as native drag/drop. A native Move up changed the order while retaining LOO-452 in the brief reader.
- Pointer drag remains unverified: automated attempts showed no order change. The final preview uses standard SwiftUI drag/drop; it is open for hands-on review. Details and process identities are in native preview notes.
- No production Work changed and no Graph was launched.
