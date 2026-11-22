# Open choices (e166ff)

Made in kickoff with nobody present; each is reversible.

- **iPhone ramp values.** display 34, title 26, lede 20, text 17, textStrong
  17, meta 13, mono 14, terminal 12. The brief fixes only the 12 pt floor and
  44 pt targets.
- **Pairing screen loses its three-icon row** (desktop ↔ phone). The headline
  "Your work, within reach." stays word for word.
- **"What do you want?" is `.serif(.display)` on the phone.** DESIGN.md's ramp
  puts it on `display`; the Mac's `NewTaskSheet` ships it on `title`.
- **The Mac's own empty and failed states are not moved to the new shared
  state view in this Task.**
- **Simulator look for the Task list, a Task and New Task needs a paired
  Machine** (`~/.lf/mobile-same-device` plus mobile access on this Mac). Build
  does not turn those on. If they are off, those three screens are checked
  through the phone tests' stand-in client and the live look moves to demo.
- `swift/README.md` describes a `--fixture` launch argument for the
  LoopflowiOS scheme that no code implements; the plan deletes the sentence
  rather than building the fixture.

Made in concept review with nobody present; each is reversible. The page is
`scratch/concept-review.html`.

- **The phone's status words become the Mac's.** "Running" → "Busy", "Ready"
  → "Available", "Open" and "No conversation yet" → the lifecycle word
  (Idle, Active, Moving, Unstarted). Passed over: keeping the phone's words
  (two vocabularies for one Task), and moving the Mac to the phone's (the
  prototypes say Available, Blocked, Moving).
- **The phone's filter names stay** ("Ready for you", "Working now"), so the
  "Working now" list holds rows that read "Busy".
- **The phone says "Auto-Ship" / "Stop Auto-Ship"**, the Mac's and the
  prototype's words, in place of "Auto on" / "Auto off". The row still says
  "Auto" and Rust's sentence says "Auto stands in…", on both surfaces.
- **The phone gains "Run"** for a Task stopped or waiting at a step, in a
  "Task actions" menu with Auto-Ship. Passed over: showing the sentence with
  no way to act on it.
- **The workflow sentence wraps on the phone** (two lines at 17 pt for a
  66-character sentence on an SE), which costs terminal rows. Passed over:
  one truncated line, and setting it in `meta`.
- **State titles keep today's one-line messages**; the state view's sentence
  is optional. "Disconnected" is the one new title.
- **The row's status written once in Rust** (word, tone, since, ask on the
  list reading) is left as its own Task; not filed.

Made in write-docs with nobody present; each is reversible.

- **The state view is named `StateView`** and the platform's control height
  `HitTarget.control`; the plan had left both unnamed.
- **The phone's Notice reads "LOO-5 Auto-Ship requested"** as the plan says,
  while the Mac's reads "LOO-5 Auto on requested · Undo". Open: which wording
  both surfaces use.
- **DESIGN.md gains a Desktop pointer** under "Two of the same thing" for its
  hand-made empty and failed states, since the phone will use `StateView`.

Made in implement (slice 1) with nobody present; each is reversible.

- **The row's status type is `TaskRowStatus`, not `TaskRow.Status`.** The
  trailing slot makes `TaskRow` generic, and a type nested in a generic is a
  different type per trailing view.
- **A status that could not be read keeps the Mac's info mark**
  (`TaskRowStatus.unread`), separate from the phone's word "Unavailable"
  (`.unavailable`, a Task whose reading lists reasons). They are two facts:
  the Mac's is a failed Sessions read, the phone's is the Task's own reading.
  Passed over: the word on both, which changes every Mac row when Sessions
  cannot be read.
- **14 pt calls**: a warning or a sentence the screen depends on went to
  `text` (17); hints, footers and paths in secondary ink went to `meta` (13).
- **The reply button's arrow** is `lede` at `imageScale(.large)`, about 26 pt
  where it was 32; its 44 pt frame is unchanged.
- **A phone row with neither a conversation state nor a lifecycle state in
  its reading shows no status word.** The old row said "Open".

Made in implement (slice 2) with nobody present; each is reversible.

- **A search with no match says "No results for “term”"** with "Clear
  search", on the Task list and the repository picker. The stock view's
  second line ("Check the spelling or try a new search.") is gone.
- **The button's side padding is a third of its height** (8 pt on the Mac as
  before, about 15 pt on the phone), so the style asks the platform one thing.
- **Secondary buttons on the phone are the neutral style** (bordered, 44 pt),
  where they were plain tinted text: "Scan another code", "Scan QR code
  instead", "Retry connection", "Save for later", "Open Settings".
- **The scanner's state leads with "Enter an address instead"** as its accent
  button; "Open Settings" is neutral beside it when the camera is denied.

Made in implement (slice 3) with nobody present; each is reversible.

- **Auto-Ship is offered to a Task the list calls working**, as "Auto on"
  was; an unstarted Task gets only Start. The Mac shows its Auto-Ship control
  on every Task and lets Rust refuse.
- **A Task with no Graph offers Start only while its row is unstarted.** The
  Mac offers Start after any reading with no Graph.
- **The phone asks for the Task from `/`**, as the Mac's `subscribeTask`
  does, not from the repository the list read it in.
- **The stop Notice reads "LOO-5 Stop Auto-Ship requested".**
- **While the Machine is away the sentence is hidden**, and comes back with
  the next reading; while the app is in the background the last sentence
  stays until the screen reads again.
- **`WorkflowLineTests` reads `WorkflowLine.swift` for `lineLimit`** to hold
  "the sentence wraps"; ViewInspector does not expose a line limit a
  container sets.

Checks (slices 1 and 2):

- `swift test --filter 'TaskRowTests|TaskStatusTests|TypographyDisciplineTests|ProgramStatusTests|TaskConceptTests|WorkPaletteTests|WorkflowLineTests'`: passed, 2026-11-22. Gate reruns.
- `uv run python scripts/test_ios.py`: TEST SUCCEEDED, 2026-11-22. CI reruns.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- Simulator look on an iPhone SE, light and dark: not done; slice 4.

Checks (slice 3):

- `swift test --filter 'WorkflowLineTests|TaskRowTests|ProgramStatusTests|WorkPaletteTests|TypographyDisciplineTests|WaveRowTests|TaskConceptTests|DesktopHeadlessTests'`: 66 tests passed, 2026-11-22. Gate reruns.
- `uv run python scripts/test_ios.py`: TEST SUCCEEDED, 45 tests, 2026-11-22. CI reruns.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- The Task screen on a phone against a real Machine (sentence, Run, Auto-Ship): not done; demo.

Made in implement (slice 4) with nobody present; each is reversible.

- **`spacing: 0` is allowed by the source scan.** It says "no gap"; there is
  no step for it and `Spacing.none` would be a name for nothing.
- **Literal → step**: 6 → `xs` inside a row and `sm` between stacked lines;
  10 and 14 → `md`; 28 → `xxl`; 18 and 20 → `xl`. The Notice's paddings
  (14, 10, 8, bottom 18) became `lg`, `md`, `sm`, `xl`, which moves the Mac's
  Notice by 2 pt on three sides.
- **The reply field's radius is `pane` (12, was 20)** and it holds a 44 pt
  minimum height.
- **44 pt frames name `HitTarget.touch`**, including the terminal's "latest"
  button; the scan does not check frames.
- **"Enter address" on pairing is the neutral button.** It was an unstyled
  button that drew as body text in the theme's ink.
- **The unavailable Session's "Changes" is its accent button** and opens the
  same sheet as the toolbar's.

Checks (slice 4):

- `swift test --filter TypographyDisciplineTests`: 2 tests passed, 2026-11-22. Gate reruns.
- `uv run python scripts/test_ios.py`: TEST SUCCEEDED, 45 tests, 2026-11-22, before "Enter address" took the neutral style; `xcodebuild build` succeeded after. CI reruns.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- Pairing on an iPhone SE (3rd generation) Simulator, iOS 26.2, light and dark: looked at, `scratch/look/pairing-{light,dark}.png`.
- Task list, a Task, New Task against a real Machine: not done; demo. `lf server mobile --status` reported ready and `~/.lf/mobile-same-device` exists, but no tool here taps in a Simulator.

Made in qa with nobody present; each is reversible.

- **The phone's Session decoder reads the three newer frame tags** (position,
  aside, close). This was broken on main, not by this Task: every held
  conversation ended in "Disconnected: Unsupported Session frame". Fixed here
  because no Task screen could be checked without it.
- **A close frame's words on the phone**: `finished` is the ended state;
  `replaced` says "This Session’s terminal was replaced."; `retired` says
  "This Session’s terminal was put away." Both offer Reconnect. Rust's CLI
  says "Session terminal replaced; reattach by Session." and "…retired;
  resume when needed."
- **"Unavailable" is the row's word only when the reading carries no state.**
  The Machine lists "No primary Session is recorded" on every Task without a
  conversation, so the earlier rule put "Unavailable" on all of Backlog.
- **Task actions read the Task's own reading** (its states and checkout), so
  they stay after Start moves the Task out of the list's filter. A write for a
  Task the list no longer shows runs from the Task's checkout.
- **An unstarted Task's screen says "No changes yet"** where it said
  "Couldn’t load: Task "task_…" is not checked out on this Machine".
- **Search fields do not capitalize** ("No results for “zzqq”", not “Zzqq”).
- **The address field is the New Task field's shape**, 44 pt tall; the system
  rounded field was 34.

Open after qa:

- The Notice sits over the Reply field for its few seconds.
- In dark, "Available" is close to the secondary ink; the tone is the shared
  palette's.
- The disposable repository left five Tasks in the Machine's store: ef7cbe
  and 299983 Done, 167dc9, 787aa8 and 2e2436 abandoned. Their repository and
  checkouts are deleted.

Checks (qa):

- iPhone SE (3rd generation) Simulator, iOS 26.2, against the installed
  Machine (lf 0.16.0+0fa6a4e20), driven with AXe: pairing, list, search,
  filters, New Task, Create & start, conversation, reply, key row, Run,
  Auto-Ship, Stop Auto-Ship, Start, Changes, diff, file, binary, Complete,
  background and return, a second view from the Mac, light, dark and
  accessibility-large text. `scratch/qa.html`.
- `swift test --filter 'OwnerStreamTests|TaskStatusTests|TypographyDisciplineTests|TaskRowTests|WorkflowLineTests|ProgramStatusTests|WorkPaletteTests'`: 51 tests passed, 2026-11-22. Gate reruns.
- `uv run python scripts/test_ios.py`: TEST SUCCEEDED, 45 tests, 2026-11-22. CI reruns.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- Not reached: landscape and iPad (AXe rotates only the Duo and physical
  devices), the QR scanner (no camera), a lost network mid-Session (the
  carrier is the person's one endpoint), a physical phone.

Made in concept review after the build with nobody present; each is
reversible. The page is `scratch/concept-review.html`.

- **The phone's Task screen opens a conversation that begins while it is
  showing**, by reading the Task's row when the Task reading lists a Session.
  Passed over: leaving it to "go back and tap again", and adding the primary
  Session to the Task reading on the wire (not needed: `task:<id>` already
  answers it).
- **Both surfaces offer Task actions by one rule, the Mac's** (`TaskActions`).
  The phone now offers Auto-Ship on every Task not completed and Start on any
  Task with no Graph, and shows Rust's refusal under a disabled action.
  Passed over: the phone's narrower rule from the list's states.
- **Both Notices say "Auto-Ship requested" and "Stop Auto-Ship requested".**
  This changes words on the Mac. Passed over: the phone saying "Auto on
  requested" for a button named Auto-Ship. This closes the open wording
  question above.
- Still open, a product choice: the filter "Working now" lists rows that
  read "Busy", and "Ready for you" lists rows that read "Available".

Made in implement (slice 5) with nobody present; each is reversible.

- **The phone sends a Task's writes from its row's `task.repo`**, as the Mac
  does (through `WaveOrigin`), in place of the list's repository path or the
  Task's checkout. Checked for Start on an unstarted Task; Run and Auto-Ship
  from a started Task's repository were not driven again after the change.
- **Before the Task's reading arrives the phone shows no "Task actions"**;
  the list row's states no longer stand in. A Task whose reading carries no
  row shows none either.
- **The Mac's workflow line reads auto from the row until the Task reading
  arrives**, as its Auto-Ship button always did; before, the line alone read
  off until then.
- **A refused action in the phone's menu is a disabled item with Rust's
  reason as its second line** (`Label` then `Text` in the button's label).
- **A row read that fails posts "The conversation couldn’t be opened: …"**
  and the next frame asks again.
- `TaskGraph.runAction` moved beside `TaskActions`; only `TaskActions` and
  `WorkflowLineTests` call it now.

Checks (slice 5):

- `swift test --filter 'TaskActionsTests|WorkflowLineTests|DesktopHeadlessTests|PendingChangesTests|TaskHistoryFilterTests|TypographyDisciplineTests'`: 56 tests passed, 2026-11-22. Gate reruns.
- `uv run python scripts/test_ios.py`: TEST SUCCEEDED, 44 tests, 2026-11-22. CI reruns.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.
- iPhone SE (3rd generation) Simulator, iOS 26.2, installed Machine, AXe:
  Backlog, an unstarted Task ("No changes yet"; menu: Start, Auto-Ship),
  Start; four seconds later the same screen showed "qa: Waiting for you at
  pause#1." over the Session terminal, and the menu read Run, Auto-Ship.
  `scratch/look/task-start-menu.png`, `task-started-conversation.png`.
- The disposable repository's Task 4afa23 is abandoned in the Machine's
  store; its repository, checkout and Simulator are deleted.
- Not driven: Run, Auto-Ship and Stop Auto-Ship after the directory change;
  a disabled action with its reason; the Mac's Notice in Loopflow Dev. qa.
