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

## Demo and layout (first cut, superseded)

The first cut's Demo, board-left Layout, Done · Active · Backlog columns and `+N
more` are replaced by the binding sections below (Session left, board right with one
selectable section, strip below). Full text: git `d9b99a6fa1`, this file. Still
current from it: tiles show the Task's live primary Session face; header click or
expand enters the Task (`.entered(from: .wave(id))`); body clicks stay in the
terminal; tiles never take focus; the page survives enter/back; `enter`, `focus -p
N/…` or sidebar selection turns the strip to the Task's page.

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

Deviations (see `scratch/questions.md` #8–#11): board rows open their Task through the Wave;
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
--since` once LOO-604 lands (`WaveStandup(closed:)` is the one input to swap).
GitHub people shipped with item 6 below.

## Concept review (October 26, after the repository page and slice 4)

The repository page and Wave views are one view (`WorkNavigation.waveView`, one
`boardTiles`/`boardPlan`, one `standupTasks`). The repository strip and the Working Tasks
Dashboard share `builtInPlan(.repository)`, so both page three across; Jack Heart kept the
query system and removed Dashboard UI entry points (`scratch/questions.md`, "Decided
October 26 — Dashboards"). Full question: git `3038a16c2c`, this file.

## Concept review (October 26, after items 1–7 and 12)

Accepted model, one screen: a Wave view (and the repository page) is three view
parts: `session` (left), `board` (right) and `tiles` (the strip, below), each hideable,
zoomable and resizable through the one `lf desktop hide|zoom|resize` state that
`view_parts` reports. The board is one Task-state vocabulary shown one section at a
time: Backlog (unstarted, deferred) · Working (idle, active, moving) · Autonomous
(shipping) · Done. Session Available/Busy is a row marker on its own axis, as AGENTS.md
puts it. People are one per email, named by GitHub once `lf repo people` resolves them.
Nothing here needs a design change: the Task-state axis, the Session axis and the
view-part grammar each have one owner.

Plan sections above "Implementation status" (Demo, Layout's board-left diagram and
Done · Active · Backlog columns, `+N more`) describe the superseded first cut;
the binding sections below supersede them. Realign rewrites them to the model above;
no code depends on them.

Strip membership (questions #17) closed October 27: Working only (Demo answers #1).

Next implement work is unchanged: `focus -p N/…` turning the strip.

## Check results

- Oct 26 (items 1–7, 12): `cargo test -p loopflow --lib -- desktop:: selection ops::people` 79 pass; `--test integration` dto/desktop/repo_people/reference/documented 48 pass; clippy clean; `scripts/test_desktop.sh --filter 'WaveStandupTests|DTOFixtureTests|DesktopPaneControlTests|WorkspacePanesTests|WorkNavigationTests|DesktopHeadlessTests|DashboardWorkspaceTests'` 156 pass. Part routing of `zoom`/`resize -p board` through the CLI has no live-Desktop test (gate/demo); `lf repo people` against GitHub ran by hand only.
- Oct 26 compress (section vocabulary, `Person.github`): `scripts/test_desktop.sh --filter 'WaveStandupTests|DesktopPaneControlTests'` 35 pass.
- Oct 27 kickoff: plan unchanged; steer 677bb5cf (no code for canceled/merged-upstream leftovers) verified in `WaveStandupBoard.swift`, no new state or filter remains. Remaining: `focus -p N/…` turning the strip; Done via `lf task list --done --since` after LOO-604.
- Oct 27 demo answers (after removing the leftover override, rerun after merging main 54932fc255): `scripts/test_desktop.sh --filter 'WaveStandupTests|WorkModelTests|WorkObservationTransportTests|TerminalPaneLayoutTests' -Xswiftc -gnone` 49 pass; `xcodebuild build-for-testing -scheme LoopflowMac` (TESTING.md) TEST BUILD SUCCEEDED; 60 s startup limit under load and board fill are for demo (Jack Heart).

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

## Demo direction — October 26, 2026 (binding; Jack Heart's words in `scratch/demo-wave-view.md`)

Built (hide/Show…, 00bc8b5d85, f504a40657): every pane and Wave part hides on
hover; `Show…` in the view header restores; `WorkModel.isHidden/setHidden` is the one
reader/writer; board/tiles hide in navigation, the conversation as its session pane.

Built (items 1–7 and 12 of the October 26 notes):
1. Dividers between conversation and board and above the strip drag (`RatioSplit` over
   `SplitDivider`); `WorkNavigation.waveRatios`; `lf desktop resize -p tiles|board
   --toward …` (Rust routes pane verbs on a part with `part=`; `-p session --toward
   board` too); `view_parts[].ratio` is the part's share.
2. Board lists scroll and load 20 more on reaching the end (`WaveBoardList`).
3. One section at a time, segmented selector, default Backlog;
   `navigation.boardSections`; `view_parts[board].section`; `lf desktop do section -p
   board section=…`; Done keeps its days control; capture lists every count, the shown
   one bracketed.
4. Conversation left (38.2%), board right, strip below (`WaveViewLayout`).
5. Zoom beside hide on every Task pane strip and Wave part; zoomed part fills the view;
   `ZoomCrumb` names it in the breadcrumb with a zoom-out button; parent crumb and Esc
   (board/strip only) zoom out; `zoom -p board|tiles` and `-p session` (its pane) agree;
   `view_parts[].zoomed`.
6. One person per email, name-only owners join by name; GitHub login/name/avatar from
   `lf repo people --json` (Rust caches per email in `github_people`, in the one draft
   migration), read once per repository by Desktop; initials until resolved.
7. Strip = Working Tasks: Rust's `wave:` expression now lists idle/active/moving, as the
   repository does; Swift's sidebar re-filter deleted.
12. Sections by Task state: Backlog (unstarted, deferred) · Working (idle, active, moving)
   · Autonomous (shipping) · Done; Working/Autonomous rows show their Session Available
   or Busy.

Remaining: `focus -p N/…` turning the strip; Done from `lf task list --done --since`
after LOO-604; the Working Tasks Dashboard now also pages Working-only Waves.

## Demo answers — October 27, 2026 (binding)

Jack Heart answered the demo questions:

1. **Strip: Working only.** The Task strip keeps the sidebar's Working rule (idle, active, moving); Autonomous Tasks stay on the board's Autonomous section only. Closes concept-review #17.
2. **No cancelled/abandoned distinction, and no code for it.** Tasks such as LOO-299 and LOO-318 that read `unstarted` locally but are canceled/merged upstream are old data. Jack: don't add code for state users can't reach; the operator edits the local store directly. Nothing to build.
3. **Fix the first-load reader timeout here.** Desktop reports "Workspace reader sent no observation for ten seconds" when the Work feed's startup runs past 10 s under load, before its first heartbeat. Make startup survive a slow first read (e.g. the feed emits a heartbeat before its first, slow projection, and/or Desktop's no-observation limit starts at the first frame) without hiding a reader that is truly hung.
4. **Board paint bug:** the board part's dark background covers only the left part of its pane (to about x 1390 of 2000). The board must fill its part's frame.

Built October 27: (1) no change; the strip already lists Working Tasks only.
(2) nothing; a `WaveStandup.state` override briefly built in 1ccee83229 was removed the
same day per Jack Heart's steer (no filtering, no new state). (3) `LocalLineObservation` allows 60 s of silence
before a reader's first frame and 10 s after it; the feed itself is unchanged (its
heartbeat already runs every 2 s once started; the stall was process startup).
(4) The board and `WaveViewHeader` fill with `.background(_, ignoresSafeAreaEdges: [])`:
the default style background ignores safe areas, which matches the capture (a fill from
the board's left edge up into the titlebar at a stale size). Confirm in the demo window.
