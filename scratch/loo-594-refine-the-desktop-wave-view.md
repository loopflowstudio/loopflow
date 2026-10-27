# LOO-594: Refine the Desktop Wave view

Status: **accepted**, October 26, 2026 (Jack Heart: "sounds good, lets build").
Remaining choices take the defaults below; `scratch/questions.md` records each.

## Problem

Jack Heart, October 25 (LOO-519 concept review): a Wave is its own kind of Desktop
view, beside Task and Dashboard views. LOO-519 (PR #1677) shipped the simplest one:
the Wave conversation on the left (62%), its Tasks as golden-layout tiles top right,
and `WaveTaskInventory` (objective, Owned/Unowned Task rows, metrics) bottom right.

Jack's October 26 direction (LOO-576 demo, `scratch/direction.md`):

- (a) The sidebar is the Task list now (LOO-576: a selected Wave moves to the top of
  the sidebar and its Tasks fill the list), so the main pane stops being a Task list.
  It becomes **the screen behind a product dev team's standup meeting**.
- (b) The Wave's Session becomes the **right pane**.
- (c) A **wide footer**: a Task selector built on the Dashboard view tech, about three
  Tasks side by side with a pager; clicking one expands the Task (and its sidebar
  Task section).

Beneficiary: Jack (and anyone he shares the screen with) working at the level of a
Wave: what moved, what's moving, what needs a person, against the chapter's KRs.

## Demo

```bash
lf open loopflow://loopflow/wave/product      # the Wave view
lf desktop state --json | jq '.windows[0] | {destination, open_work, view: .view.slots}'
lf desktop enter 4                            # footer turns to page 2, Task 4 opens in full
lf desktop back                               # back to the standup, page 2 still showing
lf desktop send -p session "what's blocked?"  # the Wave conversation on the right
```

On screen: a calm standup board filling the top left (Wave objective and chapter KRs
across the top; Done · Active · Backlog
columns below, readable from across a room), the Wave conversation on the right, and
three live Task tiles across the bottom with `Page 2 of 3`.

## Layout

```
┌──────────────────────────────────────────────┬────────────────────────┐
│ Standup board (61.8% width)                  │ Wave Session (38.2%)   │
│  product · objective (one line)              │  conversation pane     │
│  Chapter demo-20260924 · KR ✓ ✗ · targets    │  (`-p session`)        │
│  Done           │ Active │ Backlog           │                        │
├──────────────────────────────────────────────┴────────────────────────┤ 61.8%
│ Task strip: [ tile 4 ] [ tile 5 ] [ tile 6 ]          ‹ Page 2 of 3 › │ 38.2%
└───────────────────────────────────────────────────────────────────────┘
```

Golden proportions follow Jack's October 20 slot default. The footer spans the full
width (direction says "wide"); the conversation keeps the top band. **[review]**

### Standup board (main pane)

Read-only, large type (Typography strong 20+/body 15), no scrolling at a 1440×900
window with ≤ 6 rows per column; overflow shows `+N more` that enters nothing (the
sidebar and footer are the selectors). Everything derives from the Wave's
`WaveRoadmap` the model already reads (`lf wave show --json`); nothing new is stored.

| Region | Source (existing fields) |
|---|---|
| Header: Wave name, objective first line, status | `wave.displayName`, `wave.goal`, `wave.status` |
| Chapter: current Project name/status, KR checklist, metric targets | `currentProject.{name,status,krs[].holds,metricTargets}`, `metricPortfolio` |
| **Done** in the last N days. Header reads `Done`; hovering discloses `· last 7 days`; clicking the range edits the number in place (default 7, 0 = all; sticky locally in app defaults, not shared). Reuses `TaskHistoryFilter` and its inline edit view (Jack Heart, Oct 26) | `task.completed && task.completedAt ≥ now − N days`, plus `pr.phase == .merged` for still-open Tasks |
| **Active** | `section ∈ {.now, .waiting}`: identifier, name, owner summary, PR phase, `nextMove.reason`; waiting rows carry their reason inline, not a column of their own |
| **Backlog** | `section ∈ {.notStarted, .later}` and not terminal (terminal Tasks also sit in `.later`; they belong to Done), in rank order, not started first |
| Header menu: Project history, Realign Projects…, Workflow | moved from `WaveOverview` actions |

Jack Heart, October 26: the third column is **Backlog**, not "Needs a person". Jack also flagged this as the first UI that should show Tasks across people on a team; Done and Active rows get small GitHub profile pictures and names, hidden for a solo player; identity source and solo rule are proposed in `scratch/questions.md` #4, so the reasoning that follows is superseded. Previously: columns group by state, not by person: Jack is the one person and agents do the
work, so per-person standup rows would be one row. **[review]**

### Wave Session (right pane)

The Wave workspace's existing `WorkspacePanesView` (its session pane, `-p session`),
unchanged in behavior; only its place moves from left 62% to right 38.2%.

### Task strip (footer)

`DashboardSlotGrid` + `DashboardTile`, unchanged tech, members `model.waveTiles(id)`
(the Wave's planned and started Tasks in the shared order). Tiles show the Task's
live primary Session face (compact panes) as on Dashboards (decided, Jack Heart Oct 26). Header click or the expand button enters the Task
(`.entered(from: .wave(id))`): full Task workspace, crumb back to the Wave, and the
sidebar selects that Task's row. Body clicks stay inside the terminal, as on
Dashboards; tiles never take window focus (`takesFocus: false`, existing).

Paging: three slots per page from the plan's `slots`. A tile addressed by `enter`,
`focus -p N/…` or sidebar selection turns the footer to its page; the page survives
enter/back (held by the Wave view, not the grid's `@State`).

## Chosen approach

1. **Rust owns the strip geometry.** Built-in `wave-default:` plans
   (`store/sqlite/selections.rs` `view_plan`) carry a new `SlotLayout::strip(3)`
   (three vertical slots, equal weights) instead of `golden()`. Swift's `WaveTiles`
   already renders `plan.slots`, so the footer and `lf desktop state`'s `view.slots`
   agree with one writer. Saved Dashboards and unsaved queries keep `golden()`.
2. **Swift recomposes the Wave branch** of `SessionsContentView`
   (`SessionsView.swift` ≈1210): top band `HStack(WaveStandupBoard | Divider |
   WorkspacePanesView)`, then `Divider`, then `WaveTiles` at 38.2% height.
3. **New `WaveStandupBoard`** (`LoopflowMac/Views/WaveStandupBoard.swift`) with a pure
   `WaveStandup(roadmap:, now:, calendar:)` value that buckets Tasks; the view only
   renders it. The `since` rule lives in `WaveStandup` and is unit-tested.
4. **Page follows the addressed tile.** `DashboardSlotGrid` takes an optional
   `page: Binding<Int>`; the Wave view binds it to `WorkNavigation.wavePage[waveID]`
   and `enterTile`/`select(.task)` set it to `index / slots.count`. Dashboards keep
   local state.
5. **Sidebar selection in a Wave view enters through the Wave** (crumb returns to
   the standup), extending LOO-576's Dashboard rule (`navigation.trailBoard`) with a
   `.wave(id)` board. This is the "(and its Task section)" half of (c).

Pane addressing extends LOO-519 grammar (see View parts): a lone `-p` part names a
fixed view part (`board`, `session`, `tiles`) first, then a footer tile (position over the whole Wave list, never page-relative), then the Wave's own
pane; `TILE/PANE` addresses inside a tile; `enter TILE` expands; `move -p N
--destination M` reorders the shared Task order. The board and strip are view
parts with their own state and actions.

## Risk findings

- **Completed Tasks are in the roadmap.** `waves.rs` `task_section` keeps terminal
  Tasks (section `later`) with `completed_at`, sorted after open ones, so Done needs
  no new read. Stranded-Project Tasks arrive as `unavailable_tasks`; the board shows
  them as a one-line notice, as `WaveTaskInventory` does now.
- **Merged-but-open PRs carry no merge time** (`PrSnapshot` has `merge_commit`, no
  `merged_at`). With a 7-day window this matters: a merged-but-open Task would sit in Done indefinitely. Open: add `merged_at`, or show them in Active marked "merged".
- **Pager keys collide.** `DashboardSlotGrid` binds ⌘⌥←/→ to pages; LOO-519's
  `focus` action binds ⌘⌥ arrows to pane focus. Already true on Dashboards. The Wave
  strip drops the grid's keyboard shortcuts; paging stays on buttons and follows
  addressing. Dashboards' collision is out of scope (note for LOO-576/497).
- **LOO-576 landed** (#1669); slice 4 is built on its `trailBoard`.
- **Live tiles cost CPU.** `presentWaveTiles` opens workspaces for every Wave Task;
  only the visible page mounts terminal views (grid renders one page). Three live
  faces is fewer than the current golden seven. PR #1690's idle-CPU fix is the guard.
- **`view.slots` on Wave views.** The inspection's `view` must be the Wave's
  built-in plan for `lf desktop state` to report `strip(3)`; confirm at implement
  that `DesktopWindowInspection.view` is filled on `.work(wave)` destinations, and
  fill it from the same plan if not.

## View parts (draft API, Jack Heart Oct 26: board and footer must be addressable)

Direction: the `lf desktop` API adapts to new views without a new command per view.
Today a view is implicit: `-p` resolves panes on a Task, tiles on Dashboards/Wave
views, and `lf desktop do` runs registry actions scoped `window` or `pane`. A Wave
view adds two parts that are neither a terminal pane nor a tile: the board and the
strip. Proposal: every view reports its **parts**, and each part carries its own
kind, readable state and actions, so a new view is new data, not new grammar.

```jsonc
// lf desktop state --json → windows[].view_parts (new; Task/Dashboard views report theirs too)
[
  {"name": "board",   "kind": "board",   "state": {"done_days": 7},
   "actions": ["capture", "filter", "hide", "show", "zoom", "resize"]},
  {"name": "session", "kind": "session", "work": {"wave": "product"}, "actions": ["send", "capture", "..."]},
  {"name": "tiles",   "kind": "strip",   "state": {"page": 2, "pages": 3, "visible": ["LOO-594", "LOO-576", "LOO-601"]},
   "actions": ["page", "move", "enter"]}
]
```

```sh
lf desktop capture -p board            # the board as text, columns as shown
lf desktop do filter -p board days=14  # same as editing the Done header; local, sticky
lf desktop do page -p tiles 3          # footer to page 3
lf desktop enter -p LOO-601            # tiles still resolve by tile part, as today
```

- **Grammar unchanged.** `-p` gains named view parts as the first kind it matches,
  before tiles; a Task label never collides because part names are fixed words
  (`board`, `session`, `tiles`). On a Task view the parts are its three panes, so
  `view_parts` is a generalisation of what `workspaces[].layout` already lists.
- **Actions from the registry.** `DesktopActionScope` gains `part` (or `pane` widens
  to mean any part); a part lists the action names it accepts, so `lf desktop do`
  validates against the part, not a per-view table in Rust.
- **State per kind.** `board` reports its filter; `strip` its page and visible tiles
  (closes the old #7). Page stays derived: positions still count the whole Wave list.
- **Not stored.** Parts are inspection only; `done_days` lives in app defaults,
  `page` in the window. No lfdb record, no new DTO beyond the inspection fields.
- Open: does `capture -p board` return rendered text, or the bucketed JSON
  (`lf wave show` already has the data; text is what an agent "sees")?

## Alternatives considered

- **Footer under the main pane only** (conversation full height). Keeps more
  conversation, but direction says "wide footer"; held as **[review]**.
- **Swift-only strip layout** (fixed `DashboardSlotLayout` in the view). Rejected:
  `lf desktop state` would keep reporting golden slots for a view showing three.
- **Standup as a fourth Task-style pane kind.** Superseded by View parts: a board is
  a view part with its own actions, not a terminal pane.

## Wild success / failure

Success: Jack opens the product Wave before a call, shares the screen, and the board
answers "what happened, what's next, what's stuck" without narration; the
conversation beside it takes "start LOO-601" and the footer tile lights up.
Failure: the board drifts from `lf wave status` (a second bucketing rule), or live
tiles make the footer noisy and slow. Guard: one bucketing value, tested; footer
face is the Dashboard tile unchanged.

## Contract

- **Source of truth:** `WaveRoadmap` (Rust `lf wave show`) for the board; the
  built-in Wave `ViewPlan` (members, order, `slots`) for the strip; the Wave
  workspace for the conversation. All views are derived.
- **Consumers:** Desktop Wave view; `lf desktop state` (`destination`,
  `open_work`, `view.slots`); `lf desktop enter/move/focus -p` on Wave views;
  `docs/lf.md` Wave view prose and help text "Tasks are tiles beside its
  conversation" → "below".
- **Absent/error states:** planning unavailable → board shows `Wave planning
  unavailable · <reason>`, conversation and strip remain; no single current Project →
  `No current chapter Project` with KR region hidden; empty column → its title with
  `None`; no Wave Tasks → one empty slot row reading `No planned or started Tasks`;
  refresh failure keeps the last board with `Refresh failed · <error>`.
- **Exclusions:** the Portfolio window (`WavesView`/`WaveDetailPane`); Dashboard pager
  key collision; per-person grouping; standup history or snapshots. New DTO fields: only the person identity (GitHub login, name, avatar URL) on Done/Active rows, per `scratch/questions.md` #4.

## Delete — do not maintain

- `WaveTaskInventory` and its `taskGroup`/`planningNotice` (`WorkSurfaceView.swift`):
  replaced by the board. Its tests (`TaskHistoryFilterTests.swift` ≈239–260,
  `DesktopHeadlessTests.swift` ≈55) move to `WaveStandupBoard`; the behaviors they
  prove (metrics shown, unavailable-Task notice, Project label) survive there.
- `WaveTaskGroups` (Owned/Unowned), if `WaveTaskInventory` was its only consumer.
- `WaveOverview`: its fields and actions fold into the board header.
- The `case .wave` branch of `WorkSurfaceView`'s detail switch: unreachable once
  the Wave branch in `SessionsContentView` always handles active Wave keys (confirm,
  then delete).
- `golden()` for `wave-default:` plans (only that call path).

Forbidden: a second Wave bucketing rule in Rust and Swift; a Swift-side slot layout
for the strip; page-relative tile positions; a standup DTO.

## Slices

One landing. Order:

1. Rust `SlotLayout::strip(3)` for `wave-default:` plans + test in
   `selections.rs` (`wave_default_plan_uses_three_slot_strip`).
2. **This slice:** `WaveStandup` value + `WaveStandupBoard` view, replacing
   `WaveTaskInventory`/`WaveOverview`; focused test `WaveStandupTests` (bucketing,
   Monday-since-Friday window, absent states).
3. Recompose the Wave branch layout; page binding in `DashboardSlotGrid`; drop grid
   shortcuts on the strip.
4. `.wave`/`.repository` trail board so sidebar selection and tile click enter
   through the view; page follows the entered tile (built).
5. Docs: `docs/lf.md` Wave view paragraph, help/doc comments in `desktop.rs`.

## Done when

- Selecting a Wave shows the standup board left, its conversation right, three Task
  tiles below with a pager; entering a tile opens the full Task with a crumb back,
  the sidebar row selected, and back returns to the same page.
- `lf desktop state` on a Wave view reports `view.slots` as a three-slot strip and
  `open_work` in strip order; `enter`, `move`, `focus -p N/session` behave as on
  Dashboards.
- The board's Done/Active/Backlog match `lf wave status <wave>` sections
  for the same roadmap.

Acceptance at gate (headless):

- `cargo test -p loopflow wave_default_plan_uses_three_slot_strip` → passes.
- `scripts/test_desktop.sh --filter 'WaveStandupTests|DashboardWorkspaceTests|WorkNavigationTests|DesktopHeadlessTests'` → passes (board buckets a fixture roadmap; Wave view inspects `WaveStandupBoard`, `WorkspacePanesView`, `WaveTiles` with a 3-slot plan; entering a Wave tile yields `.entered(from: .wave)` and back keeps the page).
- Demo/review (Jack): the board is legible as a shared-screen standup background.

## Implementation status (October 26, 2026)

Built: slice 1 (`SlotLayout::strip(3)` for `wave-default:` plans), slice 2
(`WaveStandup` + `WaveStandupBoard`; `WaveTaskInventory`, `WaveOverview`,
`WaveTaskGroups` deleted), slice 3 (board | conversation over the strip; page held
in `WorkNavigation.wavePages`, follows `enter`, survives back; strip has no ⌘⌥
keys), slice 5 (docs/help), and View parts: `view_parts` on window inspection,
`lf desktop do ACTION k=v`, `capture -p board` (rendered text), `do page -p tiles
page=N`, `do filter -p board days=N`. Inspection `view` reports the Wave plan on
Wave views.

Deviations (see `scratch/questions.md` #8–#11): people use the claim's name and
initials (no GitHub avatars yet); board rows open their Task through the Wave;
`tiles` lists only `page`; the `.wave` case in `WorkSurfaceView` renders the board
(still reached by tests and non-terminal paths), not deleted.

Built after LOO-576 landed (October 26, Direction #2 and slice 4): the repository
page is the same Wave view (board over every repository Task with Backlog rows
that enter and offer Start, the repository Session on the right, Working Tasks
strip); `WorkNavigation.Board.repository` and Rust `DesktopBoard::Repository`;
`trailBoard` is the shown Wave view, so sidebar selection (⌘N, ⌃⇥, rows) enters
through a Wave or the repository page and the strip turns to the Task's page; the
repository built-in plan carries `strip(3)` (questions #13).

Remaining: `focus -p N/…` turning the strip; Done read from `lf task list --done
--since` once LOO-604 lands (`WaveStandup(closed:)` is the one input to swap);
GitHub identity for people.

## Concept review (October 26, after the repository page and slice 4)

Settled by accepted decisions; no design change: the repository page and Wave views
are one view (`WorkNavigation.waveView`, one `boardTiles`/`boardPlan`, one
`standupTasks` for board and capture); sidebar entry through the shown view; Done
behind one input for LOO-604. Remaining implement work is unchanged (`focus -p N/…`
turning the strip).

**Unresolved product choice (needs Jack):** the repository page's strip and the
Working Tasks Dashboard are now two views of one plan (`builtInPlan(.repository)`),
and `WorkNavigation.Board` carries both `.workingTasks` and `.repository`. Because a
plan owns one `slots`, giving the repository page its three-tile strip turned Working
Tasks from golden seven into three per page, a change Jack did not ask for
(questions #13). Question: *Now that the repository page shows the Working Tasks as
its strip, should the Working Tasks Dashboard be deleted (one view, `.workingTasks`
removed from `Board`, its picker entry and `lf desktop` destination gone), or kept as
a full-window grid of the same Tasks, in which case strip geometry must belong to
the Wave view rather than the plan (and Rust must report it from there)?* Evidence:
`selections.rs` `plan_in` default slots; `WorkModel.boardPlan` maps both boards to
one plan; `DashboardView.swift:37` still offers Working Tasks.

## Check results

- Realign (Oct 26, after slice 4): plan and code agree; no repair, no rerun. LOO-604 not yet on origin/main.
- Compress (Oct 26, repository page): the board view and `capture -p board` share `WorkModel.standupTasks(view)`; the text now carries the unavailable reason for Waves too. Swift filter (WaveStandup/TaskHistoryFilter/DesktopHeadless/WorkNavigation) → 65 pass.
- Oct 26 (repository page, slice 4): `cargo test -p loopflow --lib` desktop/selections filter → 42 pass; clippy clean; Swift filter (WaveStandup/TaskHistoryFilter/DesktopHeadless/DTOFixture/DashboardWorkspace/WorkNavigation/DesktopPaneControl/WorkDestination) → 207 pass.

- `cargo test -p loopflow --lib wave_default_plan_uses_three_slot_strip a_wave_view_names_its_board enter_names_a_tile dashboard_slots_round_trip` → pass.
- `scripts/test_desktop.sh --filter 'WaveStandupTests|TaskHistoryFilterTests|DesktopHeadlessTests|DTOFixtureTests|DashboardWorkspaceTests|WorkNavigationTests|DesktopPaneControlTests|WorkDestinationTests'` → 203 pass. `WorkOpeningCLITests` needs `target/debug/lf` built (gate).
- Compress (Oct 26): dropped the redundant part filter on `tile_request` (part verbs are never tile verbs) and the empty-verbs branch; `WaveStandup` carries `doneDays` (one default from `TaskHistoryFilter`), Backlog is two ordered filters instead of a sort. Fixed `every_verb_names_a_registry_action` to exempt `do`. `cargo test -p loopflow --lib desktop` → 21 pass; Swift filter (WaveStandup/TaskHistoryFilter/DesktopHeadless/WorkNavigation) → 61 pass.

- Oct 26 (hide/Show…): `cargo test -p loopflow --lib desktop::` 13 pass, `--test integration dto_fixtures` 42 pass, clippy clean; Swift filter (WorkspacePanes/DesktopPaneControl/WaveStandup/WorkDestination/MultiplexerStore/DashboardWorkspace/WorktreeWorkspace/DTOFixture) pass. `hide -p board` CLI routing has no live-Desktop test (gate/demo).
- Oct 26 (after `HideablePart` takes part/view/workspace, f504a40657): `scripts/test_desktop.sh --filter 'WaveStandupTests|WorkspacePanesTests|DesktopPaneControlTests'` → 37 pass.

## Direction added October 26, 2026, after acceptance (binding; supersedes conflicts above)

1. **Align with LOO-604** (branch `jack/loo-604-one-lf-api-for-loopflow-state-inventory-desktops-queries`,
   commit 3ec972e4c9, not on main). After it, `lf wave show`, `wave status`, `lf task list`
   and Desktop's planning read carry **open Tasks only**; closed Tasks come from
   `lf task list --done [--wave W] [--after ID] [--limit N] --json` (most recent first,
   `RoadmapTask` rows). `lf monitor work` is deleted; Desktop moves to `--watch` reads.
   - Done must not derive from roadmap completed rows. Keep `WaveStandup` bucketing
     behind **one Done input** (a list of closed `RoadmapTask`s) so switching to
     `lf task list --done --wave W --since` (LOO-604 adds `--since`) is a one-line change.
   - "Agents wanting structure" read `lf task list --wave W --json` plus `--done`, not
     `lf wave show`.
   - Do not fork `TaskHistoryFilter`; both branches edit it, `TaskHistoryFilterTests` and
     `DTOFixtureTests`. Whichever lands second rebases onto the other. Keep edits to
     that file minimal.
2. **Repository page is the same Wave view** (Jack Heart: "it's probably better for now
   to just make the repo page look just like a wave. It is essentially a wave").
   Selecting the repository (LOO-576's default Wave) shows the identical layout: board,
   the repository Session as the right pane, Task strip. Unfiltered: every Task in the
   repository, including unstarted backlog. Backlog offers Start where the Wave view
   does; that is where unstarted Tasks are seen and started. No waveless-only filter.
   The sidebar stays active Tasks only. A repository with no Waves: out of scope
   (LOO-621). Built (c19e51920c, cda832a1cb); the strip shows started Tasks only
   (questions #12), unstarted ones live in Backlog.

## Demo direction — October 26, 2026 (binding)

Jack Heart: "we need to make sure every pane has a hide button on hover that adds a
Show... in the task header". Every pane in Task and Wave/repository views gets a
hover hide button; hidden panes appear in a `Show…` menu in the view's header and
restore from there. Use the existing hidden-pane state and `lf desktop hide/show`
semantics (one hidden state; UI and CLI agree); `board` and `tiles` become hideable
view parts like workspace panes. See `scratch/demo-wave-view.md`.

Built October 26: every Task pane strip reveals Hide (eye.slash) on hover, focused
or not (Close stays for tabs); Wave parts wrap in `HideablePart` (hover flag on the
Wave workspace). Board/tiles hide in `WorkNavigation.hiddenViewParts`; the Wave
conversation hides as its session pane (`collapsedPaneIds`), so `hide -p session`
and the button agree. `WorkModel.isHidden/setHidden` is the one reader/writer.
`ShowHiddenMenu` sits after the Task crumb and in the board header (in
`WaveViewHeader` while the board is hidden); absent when nothing is hidden.
`DesktopViewPart.hidden` reports it; `lf desktop hide|show -p board|tiles` sends
`part=` to the pane-scoped `hide`/`show`.

## More demo direction — October 26, 2026 (binding; not yet built)

Jack Heart, October 26, 2026 (LOO-594 demo), in addition to hover-hide panes with a Show… menu in the header:

1. "we want the section borders resizable": the dividers between the board and the Wave Session, and between the top band and the Task strip, drag to resize, like Task pane dividers (same ratio state that lf desktop resize writes; reported by lf desktop state).
2. "the task lists also need to auto paginate and scroll": board Task lists scroll, and load the next page automatically as you reach the end, replacing "+N more".
3. "I prefer turning it in to just showing one section but the section is selectable. Defaults to backlog": the board shows one section at a time, Done, Active or Backlog, picked by a selector in the board header, defaulting to Backlog. The Done-days control stays on Done. The selected section is reported in the board part state (replacing done_days-only state) and settable via lf desktop do.
4. "maybe we can switch the session and the tasks pane back": the Wave/repository Session goes back on the left, the board on the right (strip stays below).
5. "but add a zoom on every pane / so that panel can expand to whole page, and add sometjhing to breadcrumb / with an easy zoom otu": every pane (Task: session, shell, browser; Wave: board, session, tiles) has a hover zoom button beside hide. Zoom fills the whole view; the breadcrumb gains a segment naming the zoomed pane (e.g. "Product › Board"), and clicking the parent crumb, a zoom-out control, or Esc returns. Same zoom state as lf desktop zoom, reported by lf desktop state.
