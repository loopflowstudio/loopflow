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
  the working colour, and "build is running. You are next needed at
  ship-review." At the narrowest width only the sentence stays.
- Auto-Ship is a plain 24 pt button.
- The Session header is one dot and a sans phrase; "New conversation;
  earlier one left behind" sits on its own line above the terminal.
- The terminal is Spline Sans Mono 13 with real bold and italics, ligatures
  off; box drawing in Claude's TUI is continuous.
- Brief opens on the Task title in Old Standard 26; Diff's file rows never
  clip.

## Acceptance check at gate

1. `lf task status 3d4350 --json | jq '.graph | {steps, sentence}'` lists
   every top-level step with its `interactive` flag and the sentence;
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
entries in `third_party/manifest.json`, `third_party/notices/app-fonts.txt`,
`third_party/notices/jetbrains_mono.txt`,
`swift/LoopflowMac/ThirdPartyNotices/{inventory.json,NOTICES.txt}` and the
tests that read them (`OpenSourceLicensesTests`, `python/tests/test_third_party.py`).

Spline Sans Mono has no box drawing, block, braille, `⎿ ❯ ✻ ●` or `→`
(probed: 0/128 box drawing). Ghostty draws box drawing, blocks and braille
itself (sprite font), so those are fine; `⎿ ❯ ✻ ●` fall back through
CoreText. QA looks at Claude's TUI for fallback width and baseline.

### Ramp (`Typography`)

`display`, `title` (serif), `lede`, `text`, `textStrong`, `meta` (sans),
`mono`, `terminal` (mono), plus `serifFamily`/`sansFamily`/`monoFamily`.
Nothing takes a size. `label`, `heroTitle`, `sectionTitle`, `body`,
`strong`, `caption`, `code` are deleted.

Serif is applied only through `.serif(.display | .title)`, a modifier that
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
"plan" or "build". Rust adds two required fields to `TaskGraph`
(`rust/loopflow-execution/src/task_graph.rs`):

- `steps: [{ name, interactive }]`: the Graph's top-level steps in walk
  order, consecutive duplicates collapsed. A concrete step's label is
  `sources[1]` (the sub-Graph it came from) or its own name. Probed on
  `feature`: explore, plan, build, ship-review, deliver, verify.
- `sentence: String`: "build is running. You are next needed at
  ship-review." / "Stopped at build. `lf task run` starts it again." /
  "Waiting for you at ship-review." / "Not started." / "At its end." With
  auto on, the clause after the first full stop names auto instead of you.
  `lf task status` text prints it in place of `summary()`.

The running step is the top-level label containing `at`. Swift's
`WorkflowLine(graph:, auto:)` draws mono name, step words, person icon,
strong working-colour running step, the sentence; `ViewThatFits` drops to
the sentence alone. It replaces `TaskWorkflowBar.dots/cells/stages/live/position`,
`WorkflowDiagram` (deleted; `WorkPalette`'s preview uses the line built from
a definition, no position), `TaskRunPosition`, and `positionLabel` in
`TaskWorkflowHeader`. `GraphCompositionView`/`GraphDiagram` stay: they draw a
Graph's definition with loops and positions no Task. Fixtures:
`tests/fixtures/dto/{task_work,task_lifecycle,task_overview,server_schema}.json`
and each language's fixture test.

### Screens

As the prototype notes say, keeping what the app can show:

- Workflow bar: line, then Auto-Ship as a neutral button (note 6).
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
dots/cells, `TaskRunPosition`, `positionLabel`. The sidebar stripe and
mono summary lines. `TaskGraph::summary()` once `sentence` replaces it.
Tests asserting on removed views (`DesktopHeadlessTests` for the bar dots,
`TaskRunPosition`, `GraphRunStep` where it changes) move to the new
components' behaviour. Must survive: every accessibility identifier UI
tests read, unless the test moves with it; Work selection/focus semantics
in DESIGN.md "Desktop semantics"; palette contrast.

## Slices

1. **Faces in the terminal and one screen (teaches most).** Bundle the nine
   files, register them, add the ramp steps and `.serif`, point the old
   helpers at the new faces temporarily, set Ghostty's family as above. Build
   the Dev app. Shows: Ghostty finds a process-registered face, bold is
   real, Claude's TUI glyphs fall back cleanly, Atkinson's wider metrics at
   the narrowest window. If Ghostty cannot see the face, the terminal plan
   changes before anything else is built.
2. **One ramp, one radius set.** Mechanical migration of every call site,
   iPhone to `MobileType`, delete the sized helpers and old radius names, add
   `TypographyDisciplineTests`. Remove old fonts and notices.
3. **Workflow line end to end.** Rust `steps` + `sentence`, fixtures, CLI
   text, `WorkflowLine`, delete the three position renderers.
4. Then: Task row + status mark + button + title bar; Session strip and
   notice line; Brief/Diff/Files; DESIGN.md once cfadc6 is on main; QA in
   three themes at the narrowest width.
