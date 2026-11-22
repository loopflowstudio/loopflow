# e166ff: Loopflow for iPhone uses the shared design tokens and components

Builds on main at 0fa6a4e202 (#2016 merged; `DESIGN.md` and
`design/prototypes/` are here). Reference: `DESIGN.md` "The language" and
"What we avoid"; `design/prototypes/desktop-more.html` (states, New Task) and
`elements.html` (Task row, workflow line).

## Outcome

Someone opening Loopflow on an iPhone sees the same Task row, status word,
workflow sentence, button and state screens as on the Mac, in the same three
faces, sized for a phone. Nothing on the phone is under 12 pt and every
control is 44 pt tall.

## What the code is today

- `swift/LoopflowiOS/MobileType.swift` takes any size: 49 calls at 12, 13,
  14, 15, 16, 17 and 24. Seven more calls use `.system(size:)` or `.caption`
  (SF Symbols, the diff and file text).
- 54 lines pass a literal padding, spacing or radius (values 2 to 28, radius
  2, 12, 16, 20). `Spacing` and `CornerRadius` are unused.
- Nine `.borderedProminent` buttons with their own tint and frame
  (`MobileNewTaskView`, `MachineConnectionView`, `SessionView`).
- `TaskListView.row` is its own row, with a colored stripe down the side
  (DESIGN.md "A stripe down the side of a rounded row").
- Ten `ContentUnavailableView`s: icon over a centred title, most with no next
  step.
- No workflow shown anywhere; the Task screen has only Start / Auto on / Auto
  off in the toolbar.
- New Task asks "What would you like to work on?" in sans 24.
- The terminal already uses the app's mono with ligatures off
  (`GhosttySurface.swift:26`), at a literal 12 scaled by Dynamic Type
  (`SessionTerminal.swift:171`), without the Mac's `adjust-cell-height = 10%`
  (`GhosttyManager.swift:132`). The diff and whole-file views use the system
  monospace.
- The "shared" components (`TaskRow`, `StatusMark`, `WorkflowLine`,
  `LoopflowButtonStyle`) live in `swift/LoopflowMac/Design/`, which the phone
  target does not compile, and `TaskRow` takes Mac-only types
  (`WorkModel.TaskPresentation`, `SessionProgramStatus`, `HoverFlag`).
- No empty or failed state component exists on any surface; the Mac draws
  each by hand.
- `swift/README.md` says the LoopflowiOS scheme opens an offline `--fixture`.
  No such argument exists in `swift/LoopflowiOS` or `swift/ios.yml`.

## Design

### One ramp, the same step names, a size per platform

`Typography` in `swift/Loopflow/Design/DesignSystem.swift` stays the one
source. Each step resolves per platform (`#if os(iOS)` … `#else`, which
`scripts/check_swift_multiplatform_boundaries.py` allows in shared code when
both branches exist). Shared components name `Typography.text` and get 13 on
the Mac and 17 on the phone. `MobileType` is deleted. DESIGN.md rule 4:
"Tokens have one source per platform and the same names on each."

The iPhone ramp (chosen here, for review):

| Step | Face | Mac | iPhone |
|------|------|-----|--------|
| `display` | Serif | 34 | 34 |
| `title` | Serif | 26 | 26 |
| `lede` | Sans 600 | 17 | 20 |
| `text` | Sans 400 | 13 | 17 |
| `textStrong` | Sans 600 | 13 | 17 |
| `meta` | Sans 400 | 11 | 13 |
| `mono` | Mono | 12 | 14 |
| `terminal` | Mono | 13 | 12 |

Reasons: 17 is the phone's body size and what the row title and reply field
already use; 13 is one above the 12 pt floor and what the row's second line
already uses; the 14, 15 and 16 pt calls go to `text` or `meta` by role
(secondary facts are `meta`, anything read as a sentence is `text`). The
terminal stays at 12 because 13 costs columns on a 375 pt screen.

Passed over: a separate `MobileTypography` enum (two sources, shared
components could not name a step); a third sans size for 15 pt (the Mac has
none, and the names would stop matching).

iPhone steps scale with Dynamic Type (`Font.custom(_, size:, relativeTo:)`),
as `MobileType` does today; Mac steps stay `fixedSize`. `.serif(_:)` scales
on the phone too. Today it is a fixed CoreText font even there.

`HitTarget` gains the platform's control height (24 Mac, 44 iPhone) under one
name so the button and row use it without asking the platform. Spacing and
radius steps are unchanged. Literal values map to the nearest step: 6 → `xs`
or `sm`, 10 → `sm` or `md`, 14 → `md` or `lg`, 28 → `xxl`; radius 12 →
`pane`, 16 → `floating`, the reply field's 20 → `pane` (the Mac's New Task
field is `pane`).

### Components move to the shared library

`TaskRow`, `StatusMark`, `WorkflowLine` and `LoopflowButtonStyle` move from
`swift/LoopflowMac/Design/` to `swift/Loopflow/Design/` and become `public`.
Passed over: a phone copy of each in `LoopflowiOS` (DESIGN.md "Two of the
same thing").

- **TaskRow** takes what it draws: title, identifier and its optional link,
  status word and tone, auto, context, the ask line, selected, muted, and a
  trailing view (the Mac's slot hint). The Mac keeps a small initializer in
  `LoopflowMac` that builds this from `TaskPresentation`, `RoadmapTask` and
  `SessionProgramStatus`; accessibility identifiers stay, so `TaskRowTests`,
  `ProgramStatusTests` and `WorkModelTests` keep passing unchanged. What the
  row needs from `WorkStyle.swift` (`WorkLinkStyle`) moves with it. The phone
  builds it from `WorkViewPlan.Content` (`title`, `identifier`,
  `status.label`, `status.tone`, `.auto` in `states`). The phone passes no
  identifier link: the whole row is the tap target. The list reading carries
  no time the status began and no ask, so the phone row shows neither; it
  does not invent them.
- **WorkflowLine** is unchanged in behavior. Its `ViewThatFits` already drops
  to the sentence alone when the steps do not fit, which is the phone in
  portrait; landscape and iPad get the full line.
- **LoopflowButtonStyle** takes its height from the platform's control
  height. On the phone the `.accent` button fills the width it is offered.
  Hover and the focus ring stay Mac behavior.
- **New: the empty or failed state**, one view in `swift/Loopflow/Design/`:
  left-aligned, a serif `title`, one sentence, then buttons. The phone's ten
  `ContentUnavailableView`s and `LoadFailure` become it. The Mac's hand-made
  states are not migrated here (see Exclusions).

### The phone's screens

- **Task list.** Rows are `TaskRow`; the stripe and the hand-made chevron go.
  Empty: title "No started Tasks" (the filter's word), sentence, "New Task"
  as the accent button. Search with no match: title, then "Clear search".
  Section header and the "N without a conversation" disclosure use `meta`.
- **Task.** One `WorkflowLine` under the navigation bar, above the terminal
  or the Changes list. The Graph comes from `lf task show <id> --json`
  (`ServedRead.taskGet`, decoded as `WorkFrame.TaskStatusReading`;
  `overview.graph.sentence` is Rust's sentence, the same one `lf task status`
  prints). Probed on this Task: `overview.graph.sentence` = "plan is running
  kickoff. Auto stands in next at ship-review." The screen reads it on
  opening and again whenever the list's reading advances (`TaskFeed.reading`),
  so it follows the store without a second subscription. `subscribeTask`
  exists on `RegistryQuery` but throws "Live Task observation is unavailable
  on this transport" where no `observeRead` is wired; if the phone's served
  transport has it, use it instead. A Task with no Graph yet shows no line.
  A failed Graph read shows no line and leaves the Notice to say why; it
  never blocks the terminal.
  Session states (elsewhere, ended, not running, disconnected) are the state
  view over the terminal with their one accent button. Changes, diff and file:
  empty and failed states are the state view ("No changes yet" gets its
  sentence; a failure keeps "Try again"); diff and file text use
  `Typography.mono`.
- **New Task.** "What do you want?" in `.serif(.display)`, the field typed in
  `lede`, one accent button ("Create & start", "Check status" or "Open
  Task"), secondary actions neutral. The destination block keeps its content
  on `Spacing` and `CornerRadius.pane`; "On your Machine" becomes `meta`.
  The dictation hint under the question stays, in `meta`.
- **Machine pairing.** "Your work, within reach." stays, in
  `.serif(.display)`. The three-icon row above it goes (it needs a 34 pt
  light symbol size the ramp does not have, and an icon over a title is the
  pattern the state rule removes). The numbered steps, address and footnote
  use `text` and `meta`. One accent button per state of the screen.
  `TextField` keeps the system rounded style only if it meets the ramp;
  otherwise it takes the New Task field's shape.
- **Terminal.** Size from `Typography.terminalSize` (12 on the phone) still
  scaled by Dynamic Type, plus `adjust-cell-height = 10%`, matching
  DESIGN.md's "ligatures off and 10% more cell height".
- **SF Symbols** take a ramp font (`.font(Typography.text)`) or
  `imageScale`; none passes a size.

Choices made here for review: the iPhone ramp values; removing the pairing
icon row; rereading the Graph on each list reading rather than subscribing;
`.display` (not the Mac sheet's `.title`) for "What do you want?".

## Source of truth

`DESIGN.md` for the system; `swift/Loopflow/Design/DesignSystem.swift` for
values; Rust's `TaskGraph.sentence` for the workflow sentence.

## Affected surfaces

- `swift/Loopflow/Design/` (tokens, four moved components, the new state view).
- `swift/LoopflowMac`: imports of the moved components, the `TaskRow`
  adapter. No visible change on the Mac; its tests are the check.
- `swift/Loopflow/Services/Notice.swift` already uses `Typography.text`; on
  the phone it grows from 13 to 17 with the ramp. Its three literal paddings
  move to `Spacing`.
- `swift/LoopflowiOS/*` views; `swift/LoopflowiOS/README.md` (terminal size
  wording); `swift/README.md` (the `--fixture` sentence, which describes
  nothing that exists).
- `DESIGN.md`: the iPhone column in the ramp under "The language"; remove the
  iPhone pointers under "Values outside the scale" and "Two of the same
  thing"; "Tokens today" gets the iPhone ramp, the components' new home and
  the state view, and loses "iPhone has no ramp of its own yet".
- No wire type changes. No Rust changes.

## Acceptance check at gate

`TypographyDisciplineTests` (source scan, headless, in `swift/LoopflowTests`)
adds `LoopflowiOS` to the directories it reads, and adds rules for that
directory: no literal `padding(`, `spacing:`, `cornerRadius:` or `EdgeInsets`
number, no `.borderedProminent`, no `ContentUnavailableView`. Expected: zero
violations. This is the check that fails if a size, a hand-made button or a
stock empty view comes back.

```sh
cd swift && swift test --filter TypographyDisciplineTests   # Mac ramp and the phone source scan
uv run python scripts/test_ios.py                           # existing phone tests, disposable Simulator
uv run python scripts/check_swift_multiplatform_boundaries.py
```

Mac component tests that must still pass after the move: `TaskRowTests`,
`WorkflowLineTests`, `ProgramStatusTests`, `WorkPaletteTests`.

## Looking at it

The brief asks to look at the Task list, a Task, New Task and Machine pairing
in the Simulator, light and dark, at the smallest supported phone. The only
installed runtime is iOS 26.2; its smallest phone is iPhone SE (3rd
generation), 375 × 667. Create one with `xcrun simctl create`, delete it by
its UDID afterwards, and stop only processes started here, by PID.

Pairing needs no Machine and can be looked at directly. The other three need
Tasks: a Simulator on this Mac is refused by the Machine until
`~/.lf/mobile-same-device` exists and mobile access (Tailscale HTTPS) is on.
Those are the person's Machine settings, so build does not change them. If
they are already on, pair and look. If not, build records in
`scratch/questions.md` that the three screens were checked from the phone
tests' stand-in client only, and the live look moves to demo.

## Absent and error states

| Where | State | Shows |
|-------|-------|-------|
| Task list | no Tasks under the filter | state view, "New Task" |
| Task list | search matches nothing | state view, "Clear search" |
| Task list | read failed | keeps the last list; the Notice says why (unchanged) |
| Task | no Graph taken up | no workflow line |
| Task | Graph read failed | no workflow line; terminal unaffected |
| Session | elsewhere / ended / not running / disconnected / unavailable | state view with its one action; "unavailable" offers Changes |
| Changes, diff, file | loading / failed / empty / binary | progress; state view with "Try again"; state view with its sentence |
| Repository picker | none on the Machine / no match | state view, "Refresh" / "Clear search" |
| QR scanner | camera denied | state view, "Enter address" (the caption blur over the camera stays) |

## Exclusions

- The Mac's hand-made empty and failed states do not move to the new state
  view here; that is Desktop work with its own prototype screens.
- Desktop's 172 literal paddings and its other "We do these now" pointers.
- No new facts on the wire for the phone row (status time, ask line).
- No iPhone theme chooser; the phone keeps the light and dark Loopflow themes.

## What goes

- `swift/LoopflowiOS/MobileType.swift`.
- `TaskListView.row` and its stripe; the hand-made chevron.
- `LoadFailure` in `TaskWorkspaceView.swift`; every `ContentUnavailableView`
  in `swift/LoopflowiOS`.
- `SessionView.card`.
- Nine `.borderedProminent` buttons with their tint and frame modifiers.
- The four component files under `swift/LoopflowMac/Design/` (moved, not
  copied).
- The pairing screen's icon row.
- The `--fixture` sentence in `swift/README.md`.

Must survive: every accessibility label and identifier (`machine-address-continue`,
`machine-connection-error`, the Mac's `work-task-*` and `task-workflow-*`),
Dynamic Type scaling on the phone, swipe to Complete, the copy on the pairing
and New Task screens other than the New Task question.

## Slices

1. **The ramp and the row, end to end.** Per-platform `Typography` and
   control height; move `TaskRow` to the shared library with its Mac adapter;
   the phone's Task list uses it; delete `MobileType` by converting every
   call to a step. Run the Mac row tests and `scripts/test_ios.py`. This is
   the slice that could prove the plan wrong: it shows whether one `TaskRow`
   serves both the Mac sidebar (hover, link, slot hint) and a phone list
   without a platform switch inside it, and whether 17/13 reads right on the
   SE. If the row needs more than a trailing-view slot to serve both, stop
   and reshape before moving the other three components.
2. **Button and state view.** Move `LoopflowButtonStyle`; add the state
   view; replace the nine buttons, the ten stock empty views, `LoadFailure`
   and `SessionView.card`. Extend `TypographyDisciplineTests` to the phone.
3. **Workflow sentence.** Move `WorkflowLine` (and `StatusMark`); read the
   Graph in the Task screen; show the line.
4. **New Task, pairing, terminal, spacing sweep.** The serif question, the
   pairing screen, terminal size and cell height, the remaining literals;
   DESIGN.md and the two READMEs; the Simulator look, light and dark.

## Done when

- The phone's Task list, Task, New Task and pairing screens show the shared
  row, the workflow sentence, "What do you want?" in serif, one accent button
  each, and left-aligned states with a next step, in light and dark on an
  iPhone SE.
- `TypographyDisciplineTests` reads `LoopflowiOS` and finds nothing.
- The existing iOS tests and the Mac component tests pass.
- DESIGN.md has the iPhone ramp, no iPhone pointer under "We do these now",
  and "Tokens today" matches the code.
