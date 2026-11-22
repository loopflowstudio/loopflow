# 3d4350 Task workspace matches the design prototype

Loopflow for Mac's Task workspace in the three new faces, on one named ramp,
built from one component each for the Task row, status mark, workflow line,
button and title bar. Stacked on LOO-515 (#1949): its Brief, Diff and Files
views are the base, restyled, never restored to what it deleted.

Reference: `DESIGN.md` and `design/prototypes/` on cfadc6's branch
`jack-heart/de-slop-fable` (read at `/Users/jack/src/loopflow.de-slop-fable/`;
not on main and not in this checkout). Screens: `desktop.html` notes 1–12,
`desktop-more.html` "The Task pane: Brief and Changes" notes 1–11.

## Demo

Build the Dev app, open this Task (3d4350) in Loopflow Light, then Loopflow
Dark, then one imported Ghostty theme, at the narrowest window:

- Sidebar rows have no stripe or border; the title wraps in full, then
  `3d4350` and its status word. Counts beside Wave names are sans.
- The workflow bar reads `code` then `plan build ship-review deliver verify` with a
  person icon before `ship-review` and `verify`, the running step strong in
  the working colour, steps before it done, and "build is running
  implement. You are next needed at ship-review." While `concept-review`
  runs inside `build`, `build` (not `plan`) is the strong step. At the
  narrowest width only the sentence stays.
- The bar's other actions are in its "…" menu, the same list as the
  sidebar row's context menu.
- Auto-Ship is a plain 24 pt button.
- The Session header is one dot and a sans phrase; "New conversation;
  earlier one left behind" sits on its own line above the terminal.
- The terminal is Spline Sans Mono 13 with real bold and italics, ligatures
  off; box drawing in Claude's TUI is continuous.
- Brief opens on the Task title in Old Standard 26; Diff's file rows never
  clip.

## Acceptance check at gate

1. `lf task status 3d4350 --json | jq '.graph | {steps, sentence}'` lists
   every top-level step with its `interactive` flag, `current_step` and the
   sentence;
   `lf task status 3d4350` prints the same sentence.
2. `swift test --filter 'WorkPaletteTests|TypographyDisciplineTests|WorkflowLineTests'`
   passes: palette contrast in every theme, no literal font size or
   `.caption`/`.system(size:)` in `Loopflow/` or `LoopflowMac/`, and the
   workflow line's words for start, running, stopped, waiting-at-you, auto
   on and end.
3. DTO fixture tests (Rust and Swift) pass with the new `TaskGraph` fields.

## Decisions and findings

### Faces (`swift/Loopflow/Fonts/`, `AppFontRegistration`)

Nine static OFL files, all from upstream, with `OFL.txt` each:
`OldStandard-Regular.ttf` (google/fonts `ofl/oldstandardtt`, PostScript
`OldStandardTT-Regular`), `AtkinsonHyperlegibleNext-{Regular,Italic,SemiBold,SemiBoldItalic}.ttf`
(googlefonts/atkinson-hyperlegible-next `fonts/ttf`), `SplineSansMono-{Regular,Bold,Italic,BoldItalic}.ttf`
(SorkinType/SplineSansMono `fonts/ttf`). Static, not the `[wght]` variable
files google/fonts ships: SwiftUI selects by PostScript name and Ghostty
finds real bold and italic faces, so nothing is synthesized. Probed with
CoreText: names above are what `Font.custom` must request; Spline Sans Mono
reports `traitMonoSpace`. Cormorant, Lato and JetBrains Mono go, with their
entries in `third_party/manifest.json` (`notice_provenance`),
`third_party/notices/app-fonts.txt`,
`swift/LoopflowMac/ThirdPartyNotices/{inventory.json,NOTICES.txt}` and the
tests that read them (`OpenSourceLicensesTests`, `python/tests/test_third_party.py`).
`third_party/notices/jetbrains_mono.txt` stays: it is GhosttyKit's own
embedded fallback face, not the app's.

Spline Sans Mono has no box drawing, block, braille, `⎿ ❯ ✻ ●` or `→`
(probed: 0/128 box drawing). Ghostty draws box drawing, blocks and braille
itself (sprite font), so those are fine; `⎿ ❯ ✻ ●` fall back through
CoreText. QA looks at Claude's TUI for fallback width and baseline.

### Ramp (`Typography`)

`display`, `title` (serif), `lede`, `text`, `textStrong`, `meta` (sans),
`mono` (mono), `terminalSize` (Ghostty's `font-size` default; Ghostty draws the cells, so no terminal `Font`), plus `serifFamily`/`sansFamily`/`monoFamily`.
Nothing takes a size. `label`, `heroTitle`, `sectionTitle`, `body`,
`strong`, `caption`, `code` are deleted.

`display` and `title` are cases of a `SerifStep` enum, not `Font` values,
so `.font(Typography.title)` does not compile. Serif is applied only through `.serif(.display | .title)`, a modifier that
sets the font (a `CTFont` with common ligatures off, feature type 1
selector 3) and tracking of −0.03 × size together, so serif without its
tracking cannot be written. `lede` is sans 600, per DESIGN.md.

Every call site migrates by this table, applied mechanically and then
looked at where the Task workspace shows it:

| Today | Step |
|-------|------|
| sans ≤ 11.5 (`caption(9…11)`, `body(11)`, `body(11.5)`, `caption()`) | `meta` |
| sans 12–16 regular | `text` |
| sans 12–14 bold | `textStrong` |
| bold 15–17, `body(17)`, `sectionTitle(15…20)` | `lede` |
| `sectionTitle(26/28)`, `heroTitle`, `strong(24)` | `.serif(.title)` |
| 34 | `.serif(.display)` |
| any `code(…)`, `.system(…, design: .monospaced)` | `mono` |
| `.caption`, `.body`, `.headline` system styles | `meta` / `text` / `textStrong` |
| SF Symbol `.system(size:)` | the step of the text beside it (+ `.fontWeight`) |

Mono is one size (12); IDs that were mono 10 grow or become sans `meta`
where they sit in prose (the prototype's sidebar ID line).

`TypographyDisciplineTests` scans `swift/Loopflow` and `swift/LoopflowMac`
sources (via `#filePath`) for `Font.custom(`/`.custom(` outside
`Design/`, `.system(size:`, `.font(.caption`, `.font(.body`, and any
`Typography.*(` call with an argument. It fails naming file and line.

**iPhone (made for review):** iPhone is out of scope but uses the deleted
helpers in 49 places at 12–24 pt; mapping it onto the Mac ramp would shrink
its body text to 13. Its calls move to a private `MobileType` in
`LoopflowiOS/` with the same sizes in the new faces, so iPhone changes face
only. The iPhone Task gives it a ramp and deletes `MobileType`; DESIGN.md's
iPhone pointer stays. Passed over: one ramp with `#if os(iOS)` sizes (a
second ramp DESIGN.md does not have).

### Radius

`CornerRadius` becomes `control` 4, `row` 8, `pane` 12, `floating` 16;
`sm/md/lg/xl/full` go (`full` callers use `Capsule`). Radius 6 (the most
used, `WorkSelection`, `WorkDestinationSelection`, `WorkOutlineButtonStyle`)
becomes `row` or `control` by kind. `PaneGeometry.cornerRadius` reads
`CornerRadius.pane`. No literal radius stays in a touched view.

### Terminal (`GhosttyManager`)

Ghostty's `font-family` is a repeatable list: a later `font-family = X`
appends a fallback rather than replacing (pinned Ghostty `a60e9e2`,
`src/config/Config.zig` lines 129–177). `loopflowConfig` loads *before* the
person's files, so a `font-family` there would beat theirs. The family line
therefore loads *after* `ghostty_config_load_default_files`, with the
embedded config: someone who set a family keeps it first and gets Spline
Sans Mono as a fallback; someone who did not gets Spline Sans Mono. The C
API cannot read `font-family` back (`RepeatableString` has no `cval`), so
this ordering is the whole mechanism. Scalar defaults (`font-size = 13`,
`adjust-cell-height = 10%`, `font-feature = -calt/-liga`) stay in
`loopflowConfig` where a person's value wins. cfadc6 adds those scalar lines
(and `font-thicken = true`) to the same constant; whichever lands second
takes the other's lines.

Open risk, answered in slice 1: Ghostty's CoreText discovery must see fonts
registered with `.process` scope. If it does not, register Spline Sans Mono
before `ghostty_init` and check again; failing that, the terminal keeps
Ghostty's default and DESIGN.md says so.

### Components

One each, in `swift/LoopflowMac/Design/` beside `WorkStyle.swift`:

- **`TaskRow`** (sidebar today: `WorkNavigator` row + `TaskRowMetadata`):
  title wraps in full (`text`, `textStrong` when selected), then ID and
  status word in `meta`. A blocked or waiting row adds one sans line with
  what the program reported (`ProgramStatusReport.msg`/`title`). No stripe,
  no border; selection is `workSelectedFill` only. PR link and owner leave
  the row (they are in the Task pane). Auto badge stays as a word.
  **Made for review:** no "since 12:07": status reports carry no start time,
  and adding one is a hook and wire change outside this Task. The row takes
  an optional time so the change that records it only fills it in.
- **`StatusMark`**: flat dot + sans phrase in the tone's ink + optional
  next-step button. Replaces `ProgramStatusLabel`, the pane strip's bare
  dot, and `WaveLensView` (glass lens, deleted with `WaveLens.glow`-only
  code). A button appears only where an action exists.
- **`WorkflowLine`** (below).
- **`LoopflowButtonStyle`** (`.neutral`, `.accent`, `.destructive`): 24 pt
  high, `text`, radius `control`. Replaces `WorkOutlineButtonStyle`,
  `DarkButtonStyle`, `GhostButtonStyle`, `DestructiveButtonStyle` on Mac.
- **`TitleBar`**: `WorkBreadcrumbs` renamed for what it is; sidebar, back,
  forward at 24 pt targets, then the jump bar, `textStrong`. RepoView and
  the backlog use it, so the backlog's capsule-and-moon bar goes if it is a
  second one.
- `WorkSectionHeading` / `WorkDisclosureHeading` lose caps and tracking:
  sentence case, `textStrong`, tertiary ink. `WorkChip` moves to `meta`.

### Workflow line: Rust owns words and steps

The wire `TaskGraph` carries interactive steps only, so the app cannot name
"plan" or "build". Rust adds three required fields to `TaskGraph`
(`rust/loopflow-execution/src/task_graph.rs`), all computed in
`read_task_graph` (`store/task_work.rs`), which holds the run's cursor:

- `steps: [{ name, interactive }]`: the Graph's top-level steps in walk
  order, consecutive duplicates collapsed. A concrete step's label is
  `sources[1]` (the sub-Graph it came from) or its own name. Probed on
  `feature`: explore, plan, build, ship-review, deliver, verify; on `code`:
  plan, build, ship-review, deliver, verify.
- `current_step: Option<u32>`: the index in `steps` of the step the Task
  stands in, from `place_of(cursor)` for a run and the interactive step's
  place for `At`; null at `start` and `end`. **Concept review:** this cannot
  be derived from `position.at` by name. `at` is `step_name_of(cursor)`, a
  bare step name, and `concept-review` is a step of both `plan` and `build`
  in `code` (this Task runs it in both), so "the label containing `at`"
  names two steps. The place is known only where the cursor is.
- `sentence: String`, written beside the index from the same place:
  "plan is running concept-review. You are next needed at ship-review." /
  "build is running. …" when the running concrete step is the label itself /
  "Stopped at build. `lf task run` starts it again." / "Waiting for you at
  ship-review." / "Not started." / "At its end." With auto on, the clause
  after the first full stop names auto instead of you. **Concept review,
  chosen for review:** the concrete step stays in the sentence. `lf task
  status` prints the sentence in place of `summary()` ("Graph code: running
  at concept-review"), and dropping the concrete step would make the CLI say
  less than it does today; the prototype's "build is running." is the case
  where the two names are one. Passed over: the concrete step only in a
  tooltip, which the CLI has no place for.

`interactive_steps` stays: it is the stop points with their skill and
description (`waitingStep`, the "Start from…" list), at a finer grain than
the line's labels, and one Rust function writes both.

Swift's `WorkflowLine(graph:, auto:)` draws mono name, step words, a
person icon before interactive ones, steps before `current_step` as done
(the prototype's `done` class: secondary ink), `current_step` strong in the
working colour while running and in waiting ink when stopped or waiting at
you, then the sentence; `ViewThatFits` drops to the sentence alone. Swift
reads no step names to place the Task. While setup has not started the run
(`task.setupPosition`, Rust's execution reason), that reason stands in for
the "Not started." sentence, as it does today. It replaces `TaskWorkflowBar.dots/cells/stages/live/position`,
`WorkflowDiagram` (deleted; the Workflow catalog shows `WorkflowSteps`, since
a definition has no top-level labels on the wire), `TaskRunPosition`, and `positionLabel` in
`TaskWorkflowHeader`. `GraphCompositionView`/`GraphDiagram` stay: they draw a
Graph's definition with loops and positions no Task. Fixtures:
`tests/fixtures/dto/{task_work,task_lifecycle,task_overview,server_schema}.json`
and each language's fixture test.

### Screens

As the prototype notes say, keeping what the app can show:

- Workflow bar: line, then the state's next step as one neutral button
  (Start when unstarted, Run when stopped: `runAction` today), then
  Auto-Ship as a neutral button (note 6), then one "…" menu. **Concept
  review, chosen for review:** the bar today also carries the lifecycle word,
  "Start from…", Complete/Reopen, Claim/Unassign, Abandon… and an Overview
  disclosure holding `WorkflowDiagram`; the prototype shows none of them and
  the plan did not say where they go. The "…" menu is `TaskActionItems`
  (the sidebar's context menu: Complete, Claim, Copy…) with "Start from…"
  and Abandon… added to it, so both places list the same actions; the
  lifecycle word is the row's and the Brief's; the Overview disclosure goes
  with `WorkflowDiagram`. Accessibility identifiers move onto the menu
  items. Passed over: keeping the buttons in a horizontal scroll, the
  clutter note 6 is about.
- Session pane strip: `StatusMark`, agent name in `textStrong`, no sparkle
  symbol; `SessionRecord`'s "New conversation; earlier one left behind" moves
  from the terminal overlay to a `meta` line above the terminal (notes 7–8).
- Files tab: one menu naming the open file and "1 of 3", place in sans,
  path mono (note 10). Markdown headings on the ramp (note 11,
  `MarkdownBlocks.swift`).
- Brief (`TaskBriefView`): `.serif(.title)` Task name; state and reason in
  one `text` sentence; PR, Related and outline as a two-column `text` table
  that wraps; "The brief changed…" at `text` in warning ink (more notes 1–4).
- Diff (`TaskChangesView`, `ReviewStreamView`): title "N changed files" with
  the step menu beside it; comparison line wraps, ref and SHA mono; delivery
  button unclipped, "Open on GitHub" without "↗"; layout choices as words;
  file rows `text`/`meta` never clipped, "12 of 85 viewed", "Viewed" as a
  word; +/− glyph beside tinted lines; per-file notes `text` with full
  stops (more notes 5–11). No "Run again" for checks: the branch has none.
  Notes 12's check rows in the workspace prototype are stand-ins.

### DESIGN.md

The pointers this change fixes and "Tokens today" are edited in DESIGN.md,
which exists only on cfadc6's unpublished branch. That edit is the last
slice and needs cfadc6 on main (then `lf sync` picks it up through
LOO-515). Until then the slice is blocked, not skipped; the PR body says so.

### Docs (written first, 2026-11-21)

`swift/README.md` (sidebar row, workflow bar and its "…" menu, Start from…,
pane header and notice line, terminal face and how a person's `font-family`
stacks, Brief, Diff title/counts/layout words/Open on GitHub, Files menu),
`docs/work.md` (`lf task status` sentence and `graph.steps`/`current_step`/
`sentence`), `swift/LoopflowiOS/README.md` (terminal face). Implement makes
the code match these words or changes both. `swift/DESIGN.md` is left alone:
cfadc6 deletes it for the root `DESIGN.md`, so its Typography and Corner
Radius snippets stay stale until that lands.

## Exclusions

New Task (`NewTaskSheet`) beyond the call-site migration, empty and failure
states outside the Task pane, Wave pages, dashboards, theme chooser,
settings, iPhone layout. They get the new faces and steps through the shared
code; their layouts follow in their own Tasks.

## What goes

Fonts: 6 old files and their notices. `Typography.label/heroTitle/sectionTitle/body/strong/caption/code`.
`CornerRadius.sm/md/lg/xl/full`. `WorkOutlineButtonStyle`, `DarkButtonStyle`,
`GhostButtonStyle`, `DestructiveButtonStyle`. `WaveLensView` and lens-only
`WaveLens` code. `WorkflowDiagram.swift`. `TaskWorkflowBar.Stage` and its
dots/cells, `TaskRunPosition`, `positionLabel`. The bar's
inline Complete/Claim/Abandon/"Start from…" buttons and its Overview
disclosure (into the "…" menu). The sidebar stripe and
mono summary lines. `TaskGraph::summary()` once `sentence` replaces it.
Tests asserting on removed views (`DesktopHeadlessTests` for the bar dots,
`TaskRunPosition`, `GraphRunStep` where it changes) move to the new
components' behaviour. Must survive: every accessibility identifier UI
tests read, unless the test moves with it; Work selection/focus semantics
in DESIGN.md "Desktop semantics"; palette contrast.

## Slices

1. **Built.** Nine faces bundled and registered; the six old files and
   their notices are gone (regenerated with `scripts/third_party.py
   refresh`). `Typography` has the ramp steps, `SerifStep` and `.serif(_:)`;
   `display`/`title`/`label` statics are gone (`WorkSectionHeading` and
   `WorkDisclosureHeading` are sentence case `textStrong` now). The sized
   helpers point at the new faces until slice 2. Ghostty: scalar defaults in
   `loopflowConfig`, `font-family` first in `embeddedConfig`; iPhone's
   Ghostty config names the family and turns ligatures off too.
   `AppBootstrapResourceTests.rampFacesAreBundled` checks every face
   resolves to the bundled file and the mono's bold, italic and bold italic
   are real. Risk answered at the CoreText level: a family-name
   `CTFontCollection` query (Ghostty's discovery path) returns all four
   process-registered Spline Sans Mono faces. Not yet seen on screen: the
   Dev app with Claude's TUI (fallback glyph width, box drawing) and
   Atkinson at the narrowest window; that look belongs to QA.
2. **Built.** Every Mac and shared call site is on the ramp (scripted by the
   table; small bold ≤ 11.5 went to `textStrong` so emphasis survives, SF
   Symbol sizes became the step plus `.fontWeight`, `.system(.body,
   design: .monospaced)` became `mono`, `.title2`/`.callout` became
   `lede`/`text`). The sized helpers are deleted; the face PostScript names
   are public so iPhone's `MobileType` (`LoopflowiOS/MobileType.swift`:
   `text(_:)`, `strong(_:)`, the old sizes) and AppKit fields name them.
   `CornerRadius` is `control/row/pane/floating`; literal radii in Mac
   views became names (4–5 control; 6–9 row, buttons and fields control;
   10–12 pane), except 1–2 pt stripe ends, which go with the stripe;
   `PaneGeometry.cornerRadius` is `pane`. `TypographyDisciplineTests`
   passes with `WorkPaletteTests`. Not run here: the iPhone build (gate's
   `scripts/test_ios.py`), and no look at the Dev app; sizes moved by up to
   ±3 pt everywhere outside the Task pane, which QA sees.
3. **Built.** `TaskGraph` carries `steps`, `current_step` and `sentence`,
   written by `TaskGraph::read` (`task_graph.rs`) from the cursor;
   `read_task_graph` hands it the run and the auto state, which
   `SavedTaskGraphRow` now reads in the same savepoint (one point query on
   `task_auto`). `summary()` is gone; `lf task status` prints
   `Graph code: <sentence>`. `lf task status --json` is `TaskDetail`, whose
   `task.graph` is the list row's `{name, step}`, so `TaskDetail` gained a
   top-level `graph` (the whole `TaskGraph`) and the acceptance `jq '.graph'`
   reads it. Seen on this Task with the branch `lf`: steps plan, build,
   ship-review (interactive), deliver, verify (interactive), `current_step` 1,
   "build is running implement. Auto stands in next at ship-review." Swift:
   `WorkflowLine` (`LoopflowMac/Design/`) draws the bar; `TaskWorkflowBar` is
   the line plus `TaskAutoControl`. Deleted: `Stage`/dots/cells/`live`/
   `autoLine`, `WorkflowDiagram.swift` (with its layout test),
   `TaskRunPosition`, `positionLabel`, `WorkflowDefinition.init(TaskGraph)`.
   The Task pane header and the Graph log print the sentence. The Workflow
   catalog inspector shows `WorkflowSteps` (New Task's existing definition
   line) rather than a definition-built `WorkflowLine`: a definition has no
   top-level labels on the wire, and deriving them in Swift would copy Rust's
   label rule. The header's Overview disclosure keeps only the brief until the
   "…" menu slice moves the header's buttons. Passed: `cargo test -p
   loopflow-execution --lib task_graph::`, `dto_fixtures::`,
   `server::schema::tests::`, and `scripts/test_desktop.sh --filter
   'WorkflowLineTests|DTOFixtureTests|TypographyDisciplineTests|DesktopHeadlessTests|RegistryQueryTests'`
   (97 tests). Not run: clippy and the lifecycle fixture regeneration
   (`task_lifecycle.json` edited by hand: its live test compares only
   position, status and Graph steps); gate.
4. **Built.** `TaskWorkflowBar` is the line, the next step (**Start** with no
   Graph, else `runAction`: **Start** at `start`, **Run** when stopped or
   waiting at an interactive step, none while running or at `end`), Auto-Ship
   (`TaskAutoControl`, a neutral `WorkOutlineButtonStyle` button reading
   **Stop Auto-Ship** while on) and "…" (`task-actions`), which is
   `TaskActionItems` over the bar's reading. `TaskActionItems` gained "Start
   from…" (or "Start with…" without a Graph), the ids `task-complete`,
   `task-reopen`, `task-claim-unassign`, and Abandon…, which sets
   `navigation.abandoning`; `TaskAbandonConfirmation` on the window asks.
   It takes the `work` reading from its host, so sidebar, Dashboard, ⌘/ and
   the bar list the same items. `WorkModel.changeRefusal` replaces three copies
   of the away/saved check. `TaskWorkflowHeader` (pane and details page) is
   the lifecycle word, the sentence, planning facts and completion conflict;
   its buttons, Overview disclosure and hover Claim/Abandon are gone. Passed:
   `scripts/test_desktop.sh --filter 'DesktopHeadlessTests|WorkflowLineTests|TaskHistoryFilterTests'`
   (30) with the new `workflowBarMenuIsTheSidebarMenu`; TaskConcept,
   SessionControls, WorkModel suites pass. `WorkNavigationTests` "Navigation
   shortcuts…" fails on ⌘⌥1 → `details brief`: the binding is in
   `DesktopActions.swift`, which no 3d4350 commit touches (last changed by
   main #1993); gate looks. Not seen in the Dev app.
5. **Built** (03c2c7403e). In `LoopflowMac/Design/`: `TaskRow` (title wraps,
   then ID link, status word in `meta` — the report's label, else the
   lifecycle word — "Auto", the ⌘N hint, and a blocked report's ask line;
   takes `since: Date?`, nil everywhere), `StatusMark` (8 pt dot + `text`
   phrase in the tone's ink + optional action; `init?(status:)` for program
   reports, `init(lens:)` for Wave/Task state via the new
   `WaveLensColor.tone`), `LoopflowButtonStyle(.neutral|.accent|.destructive)`
   and `TitleBar` (24 pt targets). Deleted: `TaskRowMetadata`, the row stripe
   and summary/excerpt line with `SessionProgramStatus.preview(now:)`,
   `ProgramStatusLabel`, the strip's bare dot, `WaveLensView` with
   `WaveLensColor.ink/isLit`, the four old button styles, the sidebar
   selection border. `TaskRowMetadataTests` is `TaskRowTests`. Passed:
   `scripts/test_desktop.sh --filter 'DesktopHeadlessTests|WorkflowLineTests|TypographyDisciplineTests|WorkPaletteTests|TaskRowTests|WaveRowTests|WaveLensTests|ProgramStatusTests|TaskConceptTests|ConsoleWorkspaceTests|WorkModelTests|WorkNavigationTests|BacklogViewTests'`
   (184 tests) except the ⌘⌥1 check above. Not seen in the Dev app.
   Realign checked: `git grep` finds none of the replaced styles or views
   outside `swift/DESIGN.md` (cfadc6's to delete).
6. **Built.** The pane strip's program menu is the agent name alone in
   `textStrong`; `PaneProgramHeading.symbol` is gone (nothing else read it).
   `ProviderTransitionRow` moved from above the whole main pane (where it
   showed the selected Session's transition whichever pane was shown) into
   `SessionPaneView`, below the strip and above the terminal, for that
   pane's own Session, `meta` in `textSecondary`. Passed:
   `scripts/test_desktop.sh --filter 'WorkspacePanesTests|DesktopHeadlessTests|ProgramStatusTests|ConsoleWorkspaceTests|TypographyDisciplineTests|ProviderTransitionTests|TaskRowTests'`
   (58) with the new `sessionStripAndNotice`. Not seen in the Dev app.
7. **Next:** Brief/Diff/Files; DESIGN.md once cfadc6 is on main; QA in three
   themes at the narrowest width.
