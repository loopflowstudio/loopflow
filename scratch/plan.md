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

"The same" means the same words and the same actions, not only the same
drawing: a Task that reads "Busy" on the Mac reads "Busy" on the phone, the
action the Mac calls Auto-Ship is called Auto-Ship, and when the sentence
says the Task waits, the screen offers the Run that moves it.

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
- The status word has two vocabularies. The Mac row says what the Task's
  program reports (`ProgramStatusState.label`: Blocked, Busy, Available,
  Done, Error) and otherwise its lifecycle (Idle, Active, Moving). The phone
  row says `WorkViewPlan.Content.status` (`WorkTone.swift`): Blocked,
  Running, Ready, Open, Unavailable, "No conversation yet". Its comment says
  "in Desktop's words"; they are not. The prototypes use the Mac's.
- One setting has three names: "Auto" on the row and in Rust's sentence,
  "Auto-Ship" / "Stop Auto-Ship" on the Mac's button and in the prototype,
  "Auto on" / "Auto off" on the phone's toolbar.
- What moves a Task is decided twice. The Mac asks the Graph
  (`TaskWorkflowBar.runAction`: Start, Run, or nothing while a run carries
  it). The phone asks the list's states and offers Start, Auto on or Auto
  off; it has no Run, so a Task stopped or waiting at a step cannot be moved
  from the phone.
- `WorkflowLine`'s sentence-alone form is one line, truncated, with the full
  sentence in a tooltip. A phone has no tooltip, and the probed sentence (66
  characters) does not fit 375 pt at 17 pt.
- The phone's link subscribes to any served read
  (`ServerLink.subscribe(_ query: ServedRead, …)`, which
  `RemoteTaskObservation` uses for the list), so one Task can be watched the
  way the Mac watches it (`subscribeTask`, `TaskWorkView.swift:481`).

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
  one status value, auto, context, selected, muted, and a trailing view (the
  Mac's slot hint). The status is one type beside the row,
  `TaskRow.Status`: the word, its tone (none for a lifecycle word), the
  detail a tooltip or VoiceOver reads, and the ask line. The row never takes
  a bare string for the word, so a caller cannot bring its own vocabulary.
  The Mac keeps a small initializer in `LoopflowMac` that builds the row
  from `TaskPresentation`, `RoadmapTask` and `SessionProgramStatus`;
  accessibility identifiers stay, so `TaskRowTests`, `ProgramStatusTests`
  and `WorkModelTests` keep passing unchanged. What the row needs from
  `WorkStyle.swift` (`WorkLinkStyle`) moves with it. The phone passes no
  identifier link: the whole row is the tap target. The list reading carries
  no time the status began and no ask, so the phone row shows neither; it
  does not invent them.
- **One status vocabulary.** `WorkViewPlan.Content.status` returns a
  `TaskRow.Status` in the Mac's words, taken from the same two sources the
  Mac row uses: `ProgramStatusState.label` for what the conversation reports
  and `TaskState.label` for the lifecycle. From the reading's states:
  `blocked` → "Blocked" (warning), `busy` → "Busy" (running), `available` →
  "Available" (available); otherwise the lifecycle state the reading
  carries (`unstarted`, `idle`, `active`, `moving`, `done`), capitalized, in
  secondary ink with no tone, exactly the Mac row's fallback. A row whose
  reading lists `unavailable` says "Unavailable" with the reasons as its
  detail (the phone has no hover for the Mac's info icon). "Running",
  "Ready", "Open" and "No conversation yet" are deleted; a Task with no
  conversation says its lifecycle word, and the list already folds those
  rows under "N without a conversation". The property moves out of
  `WorkTone.swift` to sit with `TaskRow.Status`. Passed over: carrying the
  word, the time it began and the ask from Rust on the list reading, which
  is the full fix and its own outcome (see Exclusions).
- **WorkflowLine** sets no line limit on its sentence-alone form. On the
  phone in portrait the steps do not fit, so the sentence stands alone and
  wraps in full; nothing is cut and no tooltip is needed. The Mac's
  `TaskWorkflowBar` is 34 pt high and applies `.lineLimit(1)` and its
  `.help` from outside, so the Mac looks as it does today. Landscape and
  iPad get the full line.
- **The step that moves a Task** is one function of the Graph.
  `TaskWorkflowBar.runAction` becomes `TaskGraph.runAction(issue:)` in the
  shared library beside `WorkflowLine` (label "Start" or "Run", the
  `lf task run` help line, nil while a run carries the Task or at `end`).
  The Mac's bar and the phone's Task screen both call it.
- **LoopflowButtonStyle** takes its height from the platform's control
  height. On the phone the `.accent` button fills the width it is offered.
  Hover and the focus ring stay Mac behavior.
- **New: the empty or failed state**, one view in `swift/Loopflow/Design/`:
  left-aligned, a serif `title`, then a sentence and buttons where the state
  has them. The phone's ten
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
  kickoff. Auto stands in next at ship-review." The screen subscribes to
  that read while it is on screen and the app is active, over the phone's
  own link (`ServerLink.subscribe(.taskGet(…), connect:)`, as the list
  does), and closes it on leaving; the Mac watches the same read. Rereading
  on each list frame was passed over: a step can change without the list
  changing, and each reread is the whole Task answer. A Task with no Graph
  yet shows no line. A failed or lost Graph reading shows no line and leaves
  the Notice to say why; it never blocks the terminal, and the subscription
  resumes with the link.
  The toolbar's one Task item is a menu, "Task actions", as on the Mac. It
  lists the step that moves the Task when there is one ("Start" or "Run",
  from `TaskGraph.runAction`; a Task with no Graph whose row is `unstarted`
  offers "Start") and "Auto-Ship" or "Stop Auto-Ship" (the row's `auto`
  state, as today). Each sends the write it sends today (`taskRun`,
  `taskAutoOn`, `taskAutoOff`) and the Notice names it the same way
  ("LOO-5 Auto-Ship requested"). "Auto on" and "Auto off" are deleted from
  the phone. `WorkflowLine`'s `auto:` is the row's `auto` state.
  Session states (elsewhere, ended, not running, disconnected) are the state
  view over the terminal with their one accent button. Changes, diff and file:
  empty and failed states are the state view (a failure keeps "Try again");
  diff and file text use `Typography.mono`.
  The state view's sentence and buttons are optional, so no copy is invented
  to fill a slot. A state that has one line today keeps it word for word as
  the title ("This Session is open on another device.", "The Session ended.",
  "The Session is saved, but not running.", "No changes yet", "Binary
  file"). A state that has a title and a reason keeps both ("Couldn’t load"
  and the message; "No Session here" and the reason). Disconnected has only
  a reason: its title is "Disconnected" and the reason is the sentence.
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
icon row; `.display` (not the Mac sheet's `.title`) for "What do you want?";
the phone's status words become the Mac's (Busy, Available, the lifecycle
word) and "Running", "Ready", "Open" go; the phone says "Auto-Ship" and
"Stop Auto-Ship"; the phone gains "Run" where the Graph offers it, in a
"Task actions" menu; state titles keep today's one-line messages instead of
new title-and-sentence copy.

## Source of truth

`DESIGN.md` for the system; `swift/Loopflow/Design/DesignSystem.swift` for
values; Rust's `TaskGraph.sentence` for the workflow sentence and
`TaskGraph.position` for the step that moves a Task;
`ProgramStatusState.label` and `TaskState.label` for the status word on
every row.

## Affected surfaces

- `swift/Loopflow/Design/` (tokens, four moved components, the new state view).
- `swift/LoopflowMac`: imports of the moved components, the `TaskRow`
  adapter, `TaskWorkflowBar` calling the shared `runAction` and limiting
  the line itself. No visible change on the Mac; its tests are the check.
- `swift/Loopflow/Services/Notice.swift` already uses `Typography.text`; on
  the phone it grows from 13 to 17 with the ramp. Its three literal paddings
  move to `Spacing`.
- `swift/LoopflowiOS/*` views; `swift/LoopflowiOS/README.md` (terminal size
  wording; the Task toolbar paragraph: Start or Run, Auto-Ship, the notice
  line; the workflow sentence); `swift/README.md` (the `--fixture`
  sentence, which describes nothing that exists).
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

One new headless test in `swift/LoopflowTests` holds the vocabulary: for
each of `blocked`, `busy` and `available`, `WorkViewPlan.Content.status`
gives the word `ProgramStatusState` gives the Mac row, and a reading with
none of them gives its lifecycle word. It fails if a surface gets its own
word again. `runAction`'s existing cases move with it.

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
| Task | no Graph taken up | no workflow line; "Start" if the row is unstarted |
| Task | Graph reading failed or the link dropped | no workflow line and no Start or Run; Auto-Ship stays (it needs only the row); terminal unaffected; the line returns when the link does |
| Task | a run carries it | the sentence; no Start or Run |
| Task | stopped or waiting at a step | the sentence; "Run" |
| Task list | a row's reading is unavailable | the word "Unavailable" |
| Session | elsewhere / ended / not running / disconnected / unavailable | state view with its one action; "unavailable" offers Changes |
| Changes, diff, file | loading / failed / empty / binary | progress; state view with "Try again"; state view with its sentence |
| Repository picker | none on the Machine / no match | state view, "Refresh" / "Clear search" |
| QR scanner | camera denied | state view, "Enter address" (the caption blur over the camera stays) |

## Exclusions

- The Mac's hand-made empty and failed states do not move to the new state
  view here; that is Desktop work with its own prototype screens.
- Desktop's 172 literal paddings and its other "We do these now" pointers.
- No new facts on the wire for the phone row (status time, ask line, the
  report's kind and progress). The Mac row reads Session records the list
  reading does not carry. Closing that means Rust writes the row's status
  once (word, tone, since, ask) on the list reading and both rows print it,
  with the Mac's `SessionProgramStatus` label and the Swift mapping here
  deleted. That is a wire change with DTO fixtures and a Mac migration: its
  own Task.
- The phone's filter names ("Ready for you", "Working now") stay; they name
  lists, not a row's status.
- No iPhone theme chooser; the phone keeps the light and dark Loopflow themes.

## What goes

- `swift/LoopflowiOS/MobileType.swift`.
- `TaskListView.row` and its stripe; the hand-made chevron.
- `LoadFailure` in `TaskWorkspaceView.swift`; every `ContentUnavailableView`
  in `swift/LoopflowiOS`.
- `SessionView.card`.
- Nine `.borderedProminent` buttons with their tint and frame modifiers.
- The phone's status words "Running", "Ready", "Open" and "No conversation
  yet"; the toolbar's "Auto on" and "Auto off" and its states-only choice of
  action; `TaskWorkflowBar.runAction` (moved, not copied); the line limit
  inside `WorkflowLine`.
- The four component files under `swift/LoopflowMac/Design/` (moved, not
  copied).
- The pairing screen's icon row.
- The `--fixture` sentence in `swift/README.md`.

Must survive: every accessibility label and identifier (`machine-address-continue`,
`machine-connection-error`, the Mac's `work-task-*` and `task-workflow-*`),
Dynamic Type scaling on the phone, swipe to Complete, the copy on the pairing
and New Task screens other than the New Task question.

## Slices

1. **The ramp and the row, end to end.** Built. `Typography` resolves per
   platform and `HitTarget.control` exists; `.serif(_:)` scales with Dynamic
   Type; `TaskRow`, `TaskRowStatus`, `WorkLinkStyle` and
   `ProgramStatusState.workTone` are in `swift/Loopflow/Design/TaskRow.swift`;
   the Mac's adapter and its `TaskRowSlotHint` are in
   `SessionProgramStatus.swift`; the phone's list draws `TaskRow`;
   `MobileType` is gone and `TypographyDisciplineTests` reads `LoopflowiOS`
   for fonts. Pulled forward from slice 4 because each was a font call with
   no step to name: the New Task question ("What do you want?", serif
   display), the pairing icon row's removal, diff and file text on
   `Typography.mono`.
   What it taught: one row serves both with only a trailing-view slot, so
   the other three components can move the same way. The slot makes the row
   generic, so the status type cannot nest in it (`TaskRow<A>.Status` and
   `TaskRow<B>.Status` are different types); it is `TaskRowStatus` beside the
   row. Not yet looked at on an SE: 17/13 on the row (slice 4's look).
2. **Button and state view.** Built. `LoopflowButtonStyle` and `StateView`
   are in `swift/Loopflow/Design/`; the phone's nine hand-made buttons, ten
   stock empty views, `LoadFailure` and `SessionView.card` are gone, and
   `TypographyDisciplineTests` fails on `.borderedProminent` or
   `ContentUnavailableView` in `LoopflowiOS`. The palette reaches every phone
   screen from `MobileRoot`, since shared components read it from the
   environment. Left for slice 4: the spacing rule in the scan (the literals
   are still there), and "unavailable offers Changes" as a button in the
   state (the toolbar's Changes is the only way in today).
3. **Workflow sentence and the step that moves the Task.** Next. Move
   `WorkflowLine` (and `StatusMark`) and `runAction`; the sentence wraps;
   subscribe to the Task in the Task screen; show the line; the "Task
   actions" menu with Start or Run and Auto-Ship.
4. **New Task, pairing, terminal, spacing sweep.** The New Task field in
   `lede`, the pairing screen's buttons and field, terminal cell height
   (its size already reads `Typography.terminalSize`), the remaining literals;
   DESIGN.md and the two READMEs; the Simulator look, light and dark.

## Done when

- The phone's Task list, Task, New Task and pairing screens show the shared
  row, the workflow sentence in full, "What do you want?" in serif, one
  accent button each, and left-aligned states with a next step, in light and
  dark on an iPhone SE.
- A Task shows the same status word on the phone's row as on the Mac's, and
  a Task waiting at a step can be run from the phone.
- `TypographyDisciplineTests` reads `LoopflowiOS` and finds nothing.
- The existing iOS tests and the Mac component tests pass.
- DESIGN.md has the iPhone ramp, no iPhone pointer under "We do these now",
  and "Tokens today" matches the code.

## Docs written ahead of the build

Read these before building; they describe the finished behavior.

- `swift/LoopflowiOS/README.md`: the row and its status words, the workflow
  sentence, "Task actions" (Start or Run, Auto-Ship, Stop Auto-Ship), empty
  and failed states, terminal cell height, a new "Look" section.
- `DESIGN.md`: the Mac and iPhone columns of the app ramp, the components'
  home, the two iPhone pointers removed, a Desktop pointer added for its
  hand-made states.
- `swift/README.md`: the iOS client paragraph, without `--fixture`.

Names the docs settled: the state view is `StateView`; the platform's control
height is `HitTarget.control`.

Open: the Mac's Notice for the same write reads "LOO-5 Auto on requested ·
Undo" (`swift/README.md`), and the phone's docs say "LOO-5 Auto-Ship
requested". Should the Mac's Notice say "Auto-Ship requested" too, or should
the phone keep "Auto on requested"?
