# e166ff: Loopflow for iPhone uses the shared design tokens and components

Builds on main at fd90d7e6ad, merged after slice 4; that merge touched no
Swift and no `DESIGN.md` (#2016's `DESIGN.md` and `design/prototypes/` are
here). Reference: `DESIGN.md` "The language" and
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

Slices 1 to 4 are built; slice 5 (one live Task on the phone's Task screen,
one set of Task actions for both surfaces, one name in the Notice) is not.
On top of slices 1 to 3: no line in `swift/LoopflowiOS` or `Loopflow/Services/Notice.swift`
passes a literal padding, spacing or radius, and `TypographyDisciplineTests`
fails on one in `LoopflowiOS` (`spacing: 0` passes); the phone terminal has
`adjust-cell-height = 10%`; the New Task field is typed in `lede`; the
unavailable Session state has a "Changes" button; pairing was looked at on an
iPhone SE Simulator in light and dark (`scratch/look/`). Still to do:

- Qa drove every screen on an iPhone SE Simulator against the installed
  Machine (`.lf/qa/iphone-simulator.md`, `scratch/qa.html`) and repaired what
  it found: the Session decoder's three newer frame tags (broken on main),
  "Unavailable" on every Task without a conversation, Task actions lost when
  the list's filter stops showing the Task, the unstarted Task's failed
  Changes, short diffs centred in their sheet, the 34 pt address field, the
  picker's header under its empty state, the list rows 4 pt off their headers.
- Not looked at: landscape and iPad, the QR scanner, a physical phone (demo).
- The phone's Task screen holds its Task twice: `MobileTask`, copied at the
  tap (with the conversation it had then), and `Reading`, which follows the
  Machine. A Task started from its screen keeps showing "No changes yet"
  while the sentence says a step is running; the conversation opens only
  after going back to the list (qa's worst open item).
- Which actions a Task offers is decided twice from the same reading:
  `TaskWorkspaceView.actions` on the phone, `TaskWorkflowBar.next` and
  `TaskAutoControl` on the Mac, with different rules (see slice 5).
- The Mac's Notice (`LoopflowMac/WorkModel.swift:2698`) says "Auto on
  requested" for the button named Auto-Ship; the phone's says "Auto-Ship
  requested".

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
  `TaskRowStatus`: the word, its tone (none for a lifecycle word), the
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
  `TaskRowStatus` in the Mac's words, taken from the same two sources the
  Mac row uses: `ProgramStatusState.label` for what the conversation reports
  and `TaskState.label` for the lifecycle. From the reading's states:
  `blocked` → "Blocked" (warning), `busy` → "Busy" (running), `available` →
  "Available" (available); otherwise the lifecycle state the reading
  carries (`unstarted`, `idle`, `active`, `moving`, `done`), capitalized, in
  secondary ink with no tone, exactly the Mac row's fallback. A reading's
  `unavailable` lists why no conversation opens from here ("No primary
  Session is recorded" on every Task without one), so it is the word only
  when the reading carries no state: "Unavailable", the reasons as detail. "Running",
  "Ready", "Open" and "No conversation yet" are deleted; a Task with no
  conversation says its lifecycle word, and the list already folds those
  rows under "N without a conversation". The property sits with
  `TaskRowStatus` in `TaskRow.swift`. Passed over: carrying the
  word, the time it began and the ask from Rust on the list reading, which
  is the full fix and its own outcome (see Exclusions).
- **WorkflowLine** sets no line limit on its sentence-alone form. On the
  phone in portrait the steps do not fit, so the sentence stands alone and
  wraps in full; nothing is cut and no tooltip is needed. The Mac's
  `TaskWorkflowBar` is 34 pt high and applies `.lineLimit(1)` and its
  `.help` from outside, so the Mac looks as it does today. Landscape and
  iPad get the full line.
- **What a Task offers** is one value, `TaskActions`, in the shared library
  beside `WorkflowLine`, built from what the Task's reading carries (its
  Graph, its `RoadmapTask` row and its `TaskExecution`):
  - `run`: `TaskGraph.runAction(issue:)` when there is a Graph (label
    "Start" or "Run", the `lf task run` help line, nil while a run carries
    the Task or at `end`); "Start" when a reading arrived with no Graph;
  - `auto`: on (offer "Stop Auto-Ship"), off (offer "Auto-Ship"), or not
    offered (a completed Task with auto off). On is
    `execution.auto.state == .on`, else the row's `auto` state;
  - `runRefusal`: Rust's `row.runControl.unavailable`. It disables `run` and
    "Auto-Ship" and is shown as the reason; it never disables "Stop
    Auto-Ship".
  The Mac's bar and the phone's menu both draw this value and decide nothing
  themselves. The Mac draws exactly what it draws today (the rule is its
  rule, moved). The phone changes to match: Start is offered after any
  reading with no Graph, Auto-Ship on every Task that is not completed, and
  an action Rust refuses is disabled with the reason as its second line in
  the menu instead of failing after the tap. `WorkflowLine`'s `auto:` on
  both is `TaskActions`' on. Passed over: keeping the phone's rule from
  `states` (two owners for one action, and a refusal only after the tap).
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
  lists what `TaskActions` offers: the step that moves the Task ("Start" or
  "Run") and "Auto-Ship" or "Stop Auto-Ship". Each sends the write it sends
  today (`taskRun`, `taskAutoOn`, `taskAutoOff`) and the Notice names it by
  the button's words on both surfaces: "LOO-5 Auto-Ship requested", "LOO-5
  Stop Auto-Ship requested". "Auto on" and "Auto off" are deleted as action
  names from the phone and from the Mac's Notice and its Undo help; the row's
  mark stays "Auto" (the setting), and `lf task auto on` stays the command
  the help line names.
  **The screen is one Task, read live.** `MobileTask` hands over which Task
  was tapped (ID, identifier, title) and the conversation it had then.
  Everything else the screen shows follows the Machine: the Graph, the row
  and the execution from the Task reading (`Reading` holds the `RoadmapTask`
  row, not a copy of two of its fields), and the conversation. When a frame
  of the Task reading lists a Session and the screen has no conversation, it
  reads the Task's row once (`viewPlan` with `task:<id>`, the read
  `RemoteMobileClient.createdTask` already makes; both go through one
  `task(id:directory:)`) and opens the primary Session that row names. So
  Start on an unstarted Task ends in its conversation on the same screen.
  If the row names none yet, the next frame asks again; a failed read leaves
  the Changes view and the Notice says why. Passed over: taking the
  conversation from the list's row (the list's filter hides a Task that
  just left Backlog, the case qa hit), and guessing the primary from
  `overview.sessions` (a second definition of primary).
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
"Stop Auto-Ship", and so does the Mac's Notice; the phone gains "Run" where
the Graph offers it, in a "Task actions" menu; both surfaces offer actions
by the Mac's rule, with Rust's refusal shown before the tap; the phone's
Task screen opens a conversation that begins while it is showing; state
titles keep today's one-line messages instead of new title-and-sentence
copy.

## Source of truth

`DESIGN.md` for the system; `swift/Loopflow/Design/DesignSystem.swift` for
values; Rust's `TaskGraph.sentence` for the workflow sentence and
`TaskGraph.position` for the step that moves a Task and
`TaskRunControl.unavailable` for why it cannot, both read through
`TaskActions`;
`ProgramStatusState.label` and `TaskState.label` for the status word on
every row.

## Affected surfaces

- `swift/Loopflow/Design/` (tokens, four moved components, the new state view).
- `swift/LoopflowMac`: imports of the moved components, the `TaskRow`
  adapter, `TaskWorkflowBar` and `TaskAutoControl` drawing `TaskActions` and
  the bar limiting the line itself. The one visible change on the Mac is the
  Notice's wording ("Auto-Ship requested", "Stop Auto-Ship requested");
  `DesktopHeadlessTests` and `swift/README.md` carry the old words.
- `swift/Loopflow/Services/Notice.swift` already uses `Typography.text`; on
  the phone it grows from 13 to 17 with the ramp. Its four literal paddings
  are on `Spacing`, which moved the Mac's Notice by 2 pt on three sides.
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

`TaskActions` is tested once, headless, in `swift/LoopflowTests`: waiting at
a step offers Run and Auto-Ship; a run carrying the Task offers no Run; a
reading with no Graph offers Start; Rust's refusal disables Run and
Auto-Ship and not Stop Auto-Ship; a completed Task with auto off offers no
Auto-Ship. `TaskActionsTests` in `LoopflowiOSTests` keeps only what is the
phone's: a reading the Machine could not make shows no Graph, and a Task
with no conversation opens the one its row names after a frame lists a
Session.

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
| Task | no Graph taken up | no workflow line; "Start" |
| Task | Rust refuses a run | "Start", "Run" and "Auto-Ship" disabled, the reason under each; "Stop Auto-Ship" stays |
| Task | started from its screen | the sentence, then its conversation in place of "No changes yet" |
| Task | started, but its row names no conversation yet | "No changes yet" or Changes, until a later frame's read names one |
| Task | Task reading failed or the link dropped | no workflow line and no Task actions; terminal unaffected; both return when the link does |
| Task | a run carries it | the sentence; no Start or Run |
| Task | stopped or waiting at a step | the sentence; "Run" |
| Task list | a row's reading carries no state, only reasons | the word "Unavailable" |
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
  yet"; the toolbar's "Auto on" and "Auto off"; `TaskWorkflowBar.runAction`
  (moved, not copied); the line limit inside `WorkflowLine`.
- `TaskWorkspaceView.actions(reading:states:issue:)` and its `states` and
  `directory` fallbacks to the list's row, `Reading.states` and
  `Reading.checkout`; `TaskWorkflowBar.next`; `TaskAutoControl.on`; the
  Mac's "Auto on" and "Auto off" as Notice and Undo words.
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
3. **Workflow sentence and the step that moves the Task.** Built.
   `StatusMark` and `WorkflowLine` are public in `swift/Loopflow/Design/`;
   the line sets no limit and `TaskWorkflowBar` limits it to one line with
   its tooltip; `TaskGraph.runAction(issue:)` serves both surfaces. The
   phone's Task screen subscribes with `RemoteMobileClient.observeTask`
   while it shows and the app is active, and `TaskWorkspaceView.actions`
   chooses the menu from the Graph reading and the row.
   What it taught: the Task frame decodes on the phone with the Mac's
   `WorkFrame.decode(part: .task, …)` unchanged, so there is no phone copy of
   the reading. The link's watch resumes silently after a loss, so the screen
   hides the sentence from `PendingChanges.acceptsChanges` (the Machine is
   away), not from the watch. Not yet seen on a device or Simulator: the
   wrapped sentence above the terminal (slice 4's look).
4. **New Task, pairing, terminal, spacing sweep.** Built. The literals map
   to steps as the Design section says; the reply field keeps a 44 pt
   minimum height now that its padding is `sm`. Pairing's "Enter address"
   became the neutral button after the look showed it reading as body text.
   What it taught: the docs written ahead already matched, so only the
   phone README gained a line (the unavailable state's Changes).

5. **One live Task, one set of actions, one name.** Not built. In this
   order, each showing on its own:
   1. The phone's Task screen follows the Machine for its conversation.
      Check first, on the Simulator as `.lf/qa/iphone-simulator.md`
      describes: open an unstarted Task from Backlog, Start, and the
      conversation replaces "No changes yet" without leaving the screen. If
      no frame arrives after the primary Session is recorded, note what
      does arrive and reread on that.
   2. `TaskActions` in `swift/Loopflow/Design/`, drawn by
      `TaskWorkflowBar`, `TaskAutoControl` and the phone's menu. The Mac's
      `task-workflow-run`, `task-workflow-start` and `task-auto`
      identifiers and its existing tests are the check that it looks the
      same.
   3. The Mac's Notice and Undo help say "Auto-Ship" and "Stop Auto-Ship";
      `DesktopHeadlessTests`, `PendingChangesTests` and `swift/README.md`
      follow.

## Done when

- The phone's Task list, Task, New Task and pairing screens show the shared
  row, the workflow sentence in full, "What do you want?" in serif, one
  accent button each, and left-aligned states with a next step, in light and
  dark on an iPhone SE.
- A Task shows the same status word on the phone's row as on the Mac's, and
  a Task waiting at a step can be run from the phone.
- A Task started from the phone's Task screen shows its conversation there.
- Both surfaces offer the same Task actions from one `TaskActions`, and both
  Notices say "Auto-Ship requested".
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

Slice 5 changes two sentences in `swift/LoopflowiOS/README.md`: "if the
Machine refuses, the same line says why" (a refusal Rust already knows is
shown under the disabled action; the line covers the rest), and the Changes
paragraph gains that a Task started from its screen opens its conversation
there. `swift/README.md` still reads "LOO-5 Auto on requested · Undo".
