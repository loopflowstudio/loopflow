# New Task and every empty or failed state match the prototype

Task 923677. Reference: `DESIGN.md` ("The language", "What we avoid") and the
"New Task" and "When nothing is there, and when something fails" sections of
`design/prototypes/desktop-more.html` (open it; it is live). Both are on main
since #2016; this builds on main.

## Outcome

Someone pressing ⌘N in Loopflow for Mac sees the prototype's sheet: "What do
you want?" in 34 pt serif, their request at 17 pt, "In loopflow, running
feature, loopflow's default" with two ordinary menus, the workflow's steps as
the same line a Task shows later, "You take part at explore, ship-review and
verify." under it, and one accent button reading "Start ⌘↩". (The prototype
says "human-review"; step names are whatever Rust reports.)

A Task filed and not yet started says the same thing in the same words: its
bar draws that step line, and its pane reads "Not started. It will run
feature, Product's default. You take part at explore, ship-review and
verify." with Start and "Start with…". (The prototype says "the Wave's
default"; the wire cannot tell a Wave's `graph:` from the repository's, so the
pane names the scope as the sheet does. Open choice, questions 13.)

Anywhere the app has nothing to show or something failed, the pane is
left-aligned: a 26 pt serif title saying what happened, one sentence, then
buttons for what to do next. No grey icon over a centred title anywhere in
`swift/LoopflowMac`. A failure names what failed, shows the machine's message
in mono, and keeps Retry.

## Demo

`uv run python scripts/loopflow-dev.py run`, then:

1. ⌘N. Type a request, switch the workflow menu from the default to another:
   the step line and the "You take part at …" sentence change with it.
2. Open an empty repository: "No planned Work in this repository", with
   "New Task ⌘N" and "Open the repository conversation" to press.
3. Open a path that is not a repository: "This repository did not open", the
   error in mono, Retry.

Each in Loopflow Light and Loopflow Dark.

## Findings that shape the design

**No shared state component exists yet.** `DESIGN.md` lists "Empty or failed
state" as one component, but `swift/LoopflowMac/Design/` has only `TaskRow`,
`StatusMark`, `WorkflowLine`, `LoopflowButtonStyle` and `TitleBar`. The closest
thing is `PaneEmptyState` (`Views/WorkspacePanesView.swift:186`), a wrapper
around `ContentUnavailableView` with a one-line compact form for dashboard
slots, used at 7 sites. The brief's "Use them; add none" cannot hold for this
one. **Choice made for review:** `PaneEmptyState` becomes the component. It
moves to `Design/EmptyState.swift`, renamed `EmptyState`, and its full form is
rewritten to the prototype's `.app.state` (`desktop.css:223-228`). No second
type is added. The name is not `StateView`: "state" already means a Task's
state and `PaneState`, a pane's layout. Passed over: a new component beside `PaneEmptyState` (two of
the same thing), and styling `ContentUnavailableView` in place (it centres and
wants an icon).

**A Graph definition has no top-level steps on the wire, and Rust already
knows how to make them.** `WorkflowLine` draws `TaskGraph.steps`, which
`CapturedGraph::workflow()` (`rust/loopflow-execution/src/task_graph.rs:329`)
derives from `&[ConcreteStep]`: composed steps read as the Graph they came
from, consecutive repeats collapse. The catalog (`graph/compiled.rs:155`,
`load_catalog_entry`) holds the same `Vec<ConcreteStep>` from
`flatten_resolved` just before it compiles it. So the catalog can carry the
same steps with no second derivation. `WorkflowSteps` exists only because
this field was missing; Swift's `WorkflowDefinition`
(`swift/Loopflow/Models/Graph.swift:256`) re-derives a different shape
(interactive steps and unnamed runs) from the compiled nodes.

**"Where you take part" is derived three ways, and they disagree.**
`TaskGraph.interactive_steps` walks the whole Graph, XOR paths included
(`task_graph.rs:144`); a started Task's sentence ("You are next needed at …")
uses it. `workflow()` marks a top-level step interactive only when the step
itself is an interactive skill (`task_graph.rs:345`), so a stop inside an XOR
path gets no person icon. `StartFlow.stops` (`BacklogView.swift:122`) counts
interactive nodes in Swift. For `feature` and `code` all three agree. For a
Graph whose only stop is inside an XOR path the menu says "1 stop", the line
shows no person, and a sentence built from the top-level flags would say
"Runs on its own." The outline therefore carries the whole-Graph list and
everything reads that.

**A filed Task already knows the workflow it will run.**
`RoadmapTask.graph_name` and `graph_source` are "the graph the Task took up,
else the one its first `lf task run` takes up" and the layer that chose it
(`waves.rs:491`). Swift phrases that pair four ways: `defaultPhrase` in the
sheet, "feature · repository" (`TaskWorkView.swift:26`), "Takes up feature
when it starts" (`TaskWorkView.swift:180`), and "No workflow selected" for a
case Rust does not send. The sheet's `WaveGraph.Default` is the same pair for
a Task that does not exist yet.

**The unstarted Task's pane is `TaskStartOffer`** (`TaskWorkView.swift:233`),
through `SessionSlotOffer` (`WorkspacePanesView.swift:143`, its only caller),
shown in the Task's pane, `SessionsView:1738` and dashboard tiles
(`DashboardView:236`). "Conversation not open" (`WorkspacePanesView:93`) is
the branch for every other Work. Choosing a workflow there starts the Task
(`startTaskFromInput($0)`); nothing stores a choice without starting.

**`StartFlowItems` (`BacklogView.swift`) is the workflow list as menu
items**: the Task bar's "Start with…" and, since slice 2, the sheet's
workflow menu. The unstarted pane's "Start with…" uses it too.

**"Open the repository conversation" is a real destination**
(`Destination.repository`, `WorkDestination.swift:8`), New Task is
`model.navigation.palette = .newTask`, and "Read the plan again" is
`model.rescanWork()` (`WorkModel.swift:1648`). ⌘K is bound to the navigator
(`SessionsView.swift:43`).

## Source of truth

- Workflow steps and the take-part sentence: Rust, on `GraphCatalogEntry`.
- Layout and wording of the sheet and the drawn states: the prototype.
  States it does not draw follow the same form with wording from `DESIGN.md`'s
  Text rules (sentence case, digits, no middle dot, no em dash).

## Design

### Wire: `GraphCatalogEntry.workflow`

```rust
/// A Graph as the workflow line shows it before any Task stands on it.
pub struct WorkflowOutline {
    pub steps: Vec<WorkflowStep>,
    /// Every step where a person takes part, as `TaskGraph` carries them.
    pub interactive_steps: Vec<InteractiveStep>,
    /// "You take part at explore, ship-review and verify." or "Runs on its own."
    pub sentence: String,
}
// GraphCatalogEntry
/// Null exactly when `compiled` is.
#[serde(deserialize_with = "Option::deserialize")]
pub workflow: Option<WorkflowOutline>,
```

`workflow()` and `interactive_summary()` become functions over
`&[ConcreteStep]` that both `CapturedGraph` and `load_catalog_entry` call, so
an outline is the part of a `TaskGraph` that needs no Task. The sentence
names `interactive_steps`, the list a started Task's sentence draws on.
**Choice made for review:** a top-level step is `interactive` when any step
inside it is, XOR paths included, so the line, the sentence and the stop
count agree; a started Task's line gains a person icon on such a step.
Passed over: a sentence from the top-level flags as they are (it can say
"Runs on its own." about a Graph that stops). `StartFlow.stops` becomes
`interactive_steps.count`; its `merges` keeps its walk over compiled nodes.
Swift mirrors it with `decode(WorkflowOutline?.self, forKey:)`;
`tests/fixtures/dto/graph_catalog.json` and both languages' fixture tests gain
the field. `lf graph list --json` carries it; the text listing is unchanged.

### `WorkflowLine` for a definition

`WorkflowLine` gains a second way in: an outline with no position (every step
reads as ahead, none strong). Three places use it:

- The sheet: the steps wrapping on their own row and the sentence on the row
  below in `textSecondary`; the Graph's name is not repeated because the menu
  above already says it.
- An unstarted Task's bar: the name in mono and the steps, from the catalog
  entry named by `task.graphName`, in place of "Takes up feature when it
  starts". Until the catalog has loaded, or when it has no such entry, the
  bar shows the name alone. "No workflow selected" goes.
- `WorkflowCatalogInspector` (`WorkPalette.swift:429`).

A started Task's bar is unchanged.

### Whose choice the workflow is

Built: `WorkModel.workflowChoice(source:wave:)` ("Product's default", "your
default", "the default", "for this Task only") and `workflowSelection(_:)`
for a filed Task's pair. The sheet, the unstarted pane, the status strip and
the Graph log's position line read them. They are at the end of `WorkModel.swift`.

**Open choice (questions 13):** saying "the Wave's default" as the prototype
does needs Rust to send which layer's `graph:` won (a new `ConfigSource` or a
scope field). Until someone asks for it the scope's name stands.

### `EmptyState`

Built: `Design/EmptyState.swift`,
`EmptyState(title:sentence:machine:) { actions }`. Leading, vertically
centred, serif title, sentence about 52 characters wide, the machine's
message in mono and selectable, `LoopflowButtonStyle` buttons. Compact
(`\.compactPanes`): the title and the buttons on one line, else the buttons
alone with the sentence or message as help. No `systemImage`.

A compact state with no button keeps its title, cut short where the slot
ends. Beside it in the same file: `.emptyLine()` for a list with nothing in
it, and `FailureLine` (what failed, the message in mono, the way to retry)
for a failure beside content that stays.

### New Task sheet

Built (slice 2); `docs/surfaces.md` "New Task" is the wording and
`NewTaskSheet.swift` the layout. Still to look at in the Dev app: the
borderless menus' face and baseline beside the sentence (the commas are
pulled back with a negative leading pad), and the sheet's height.

**Choices made for review:**
- The workflow menu is `StartFlowItems`; `WorkflowChooser` goes, and with it
  the search field and each workflow's summary line. The brief asks for
  "ordinary menus"; passed over: keeping the popover behind a menu-shaped
  button.
- The revert arrow goes; choosing the default in the menu reverts.
- File stays (the prototype draws only Cancel and Start, but filing without
  starting is shipped behaviour from #1993 and the brief does not remove it).

`StartingTaskView` has the prototype's failed-start parts (the request at
`Typography.lede`, "Not started.", the error in mono, "Try again" accent and
"Discard") but sits top-leading in a scroll view. **Choice made for
review:** it stays that way and does not become an `EmptyState`: before a
failure it shows only the request, where the conversation is about to
appear, and a failure should not move those words to the pane's middle.

### The states

The prototype's drawn states are built: `NoPlannedWork` and `ChooseWork`
(`WorkSurfaceView.swift:9`, `:32`), the missing selection (`:240`),
`RepositoryDidNotOpen` (`RepositoryWorkspaceView.swift:98`),
`TaskStartOffer` (`TaskWorkView.swift:236`) and the pane sites in
`WorkspacePanesView.swift`. `docs/surfaces.md` holds their wording.

**Choices made for review:**
- The second button reads "Start with…", not the prototype's "Choose
  workflow": picking an item starts the Task, and "Start with…" is what the
  "…" menu and the backlog row call the same items. Passed over: a "Choose
  workflow" that stores without starting (a new action).
- An unstarted Task's full screen has Start twice, neutral in the bar and
  accent in the pane. The bar's stays because it is the one left when the
  conversation pane is collapsed or another pane is zoomed.
- "Open another repository…" and "Open its conversations" did not ship: the
  open panel lives in `LoopflowApp`, and neither view reaches it or a
  retained workspace's conversations (questions 8).
- The unstarted pane shows neither the Task's description nor the refusal
  under Start; a refused Start is off with the reason as its tooltip
  (questions 17). The workflow's name in its sentence is not mono
  (questions 16).

Not drawn; same form, sentence case, next step only where the app has one.
All built:

| Site | After |
|------|-------|
| `RoadmapView:160` | "Nothing needs action" / "No live or stopped work across these Waves." / Show roadmap (the lens switch) |
| `DashboardView:127` | "This dashboard is no longer available" / Show working Tasks |
| `SessionsView:1440` | "This repository did not open", `machineError` in mono, Retry only if a re-read exists there |
| `WorkActivityView:86` | "No activity in this window" / "Work creation, runs, PRs and steers appear here." |
| `TaskFilesView:420` | "Select a file" |
| `TaskFilesView:425` | "Comparison unavailable", reason in mono, Retry through the store's existing retry if it covers changes |
| `TaskFilesView:435` | the snapshot state's title |
| `OpenSourceLicenses:51` | title and sentence as they are; nothing to press |

Plain `Text` empties stay one line (one serif thing per screen): sentence
case, `Typography.text`, `palette.textSecondary`, full stop, and a button
beside the sentence where the app has the action.

- Gains a button: "No planned or started Tasks." (New Task); "No Graph has
  run." (Start, for a Task awaiting its first start, under the bar's refusal
  rule); "No recorded work." (its Refresh, set beside it). "No Tasks in this
  Project.", "No current Tasks." and "No Projects in this Wave." have no
  `WorkModel` to open New Task from: wording only (questions 20).
- Wording only: "No pull request yet · changes below are local"
  (`PullRequestStore.swift:136`) loses its middle dot, as do
  "Project unavailable · …", "Workflows unavailable · …"; "No comments.",
  "No brief yet.", "No description recorded.", "No Session history recorded
  for this Task.", "No matching files", "No matches", "No files to show",
  "No steps", "Empty comment", "Task status unavailable." get the face,
  colour and full stop where they differ. "Task is unavailable" and "Session
  location unavailable" fill a pane and are `EmptyState` titles.
- Inline failures (`RepositoryWorkspaceContent`'s error beside a retained
  workspace, "Latest read failed: …", `TaskFilesView:297`,
  `ReviewStreamView:69`, `SessionsView:1787`, `WorkPalette` source errors,
  `RemoveWaveSheet:74`): what failed in sans, the machine's message in mono,
  Retry kept.

## Absent and error states

- Catalog still loading or failed: the sheet shows the sentence with "…" for
  the workflow and no step line, as today; Start still works (Rust picks the
  default).
- An entry whose definition does not load has `workflow: null` and stays out
  of the sheet's menu, as `compiled == nil` does today.
- A workflow with no interactive step: the line of steps, then "Runs on its
  own."
- An app built from this branch against an `lf` without the field fails to
  decode the catalog, which the app already reports as "Workflows
  unavailable". The Dev app check needs the branch's `lf`.

## Exclusions

- Loopflow for iPhone's `ContentUnavailableView`s (10 sites).
- Loading states ("Opening repository…" stays).
- The settings form, the Wave page, the two Create Wave forms, the 172
  literal paddings outside the files this touches.
- `lf graph list`'s text output.
- New actions. A state gets a button only for something the app already does.
- Storing a workflow for a filed Task without starting it: it needs a
  Task-level action in Rust; a separate Task if wanted.
- "Merges without asking" stays worked out in Swift from op labels
  (`BacklogView.swift:127`); it disagrees with nothing shown here.

## What goes

Gone: `WorkflowSteps`, `WorkflowChooser`, `WorkflowDefinition`,
`WorkflowCatalogEntry`, `model.workflowCatalog`, `StartFlow`'s own stop
count, "Takes up … when it starts", the "· loopflow's default" assertion,
the `new-task-workflow-<name>` identifiers, `PaneEmptyState`,
`SessionSlotOffer`, `NewTaskWorkflow.defaultPhrase`, "feature · repository",
"No workflow selected" and every `ContentUnavailableView` in
`swift/LoopflowMac`. Remaining: the two prototype sections, after the Dev
app check.

Must survive: File without starting, ⌘↩ and Esc, the sheet opening on the
sidebar's Wave, the default following the scope, the request kept when
filing is refused, every accessibility identifier tests and `lf` Desktop
actions use other than the two named above, compact dashboard slots staying
one line.

The prototype sections stay until the screens are checked against them in
the Dev app (`DESIGN.md`: "Delete a prototype when its screens ship").

## Acceptance check at gate

```bash
cargo test -p loopflow-execution --lib graph::compiled::
cargo test -p loopflow --test dto_fixtures
scripts/test_desktop.sh --filter NewTaskTests
scripts/test_desktop.sh --filter EmptyStateTests
scripts/test_desktop.sh --filter TypographyDisciplineTests
lf graph list --json | jq '.[] | select(.name=="code") | .workflow'
```

The last prints `steps` plan, build, ship-review, deliver, verify with
`interactive` true on ship-review and verify, `interactive_steps` naming
those two, and `"sentence": "You take part at ship-review and verify."`

View tests (ViewInspector, headless):
- Rust: a Graph whose only stop is inside an XOR path has that top-level
  step `interactive`, an outline sentence naming the stop, and
  `interactive_steps` equal to a `TaskGraph` read of the same Graph.
- Unstarted Task: its pane shows the title, the outline's sentence and a
  Start that calls `startTaskFromInput`; its bar shows the step names.
- Sheet: "What do you want?" found; choosing another workflow changes the
  step names and the take-part sentence; the Start button's label holds
  "⌘↩"; Start and File disabled on an empty request.
- Empty: `WorkSurfaceView` on a repository with no Work shows the title, the
  sentence, and a New Task button that sets `navigation.palette == .newTask`.
- Failed: `RepositoryWorkspaceContent` with an error shows the title, the
  error text, and a Retry that calls `retry`.
- Source scan beside `TypographyDisciplineTests`: no `ContentUnavailableView`
  under `swift/LoopflowMac`.

Dev app in Loopflow Light and Loopflow Dark, each state reachable from the
demo: not headless; qa or demo runs it.

## Slices

1. **The wire and the sheet's step line. Built.** `WorkflowOutline` is on
   `GraphCatalogEntry.workflow`; `WorkflowLine` draws it in the sheet (steps
   wrapping, sentence under), the inspector and an unstarted Task's bar (name
   and steps). `WorkflowSteps`, `WorkflowDefinition`, `WorkflowCatalogEntry`,
   `model.workflowCatalog`, "Takes up … when it starts" and `StartFlow`'s own
   stop count are gone. What it taught:
   - An outline equals a started Task's reading (`graph::compiled::tests::
     a_catalog_entry_outlines_the_workflow_a_task_would_show`: `code`, and a
     Graph whose only stop is inside an XOR path).
   - The sentence names each stop once, so a stop repeated across XOR paths
     does not read twice; `interactive_steps` keeps every one.
   - Wrapping needed a small private `Layout` in `WorkflowLine.swift`
     (`WrappingRow`); SwiftUI has none. ViewInspector reads through it.
   - Not seen: the line at 34 pt bar height and in the 540 pt sheet in the
     Dev app. Headless tests cover the content only; qa or demo looks.
   - `TaskWorkflowHeader` still says "feature · repository" and "No workflow
     selected" (`TaskWorkView.swift:26`, `:418`), and
     `WorkNavigationTests.swift:1013` rebuilds that phrase; slice 3's
     whose-default phrase replaces all three.
   - Checks: `cargo test -p loopflow-execution --lib graph::compiled::`
     passed at 00360b36ba (10 tests). The Swift suites
     (`WorkflowLineTests`, `NewTaskTests`, `WorkNavigationTests`) and
     `dto_fixtures` have no recorded result; gate runs them.
2. **The rest of the sheet. Built.** The question is `.serif(.display)`, the
   request `Typography.lede`, the sentence two `.borderlessButton` `Menu`s
   (the workflow one is `StartFlowItems`), Start is the accent
   `LoopflowButtonStyle` reading "Start ⌘↩", paddings are `Spacing` steps,
   and "Not filed." and "Not started." are strong in `palette.failure` with
   the reason in mono. `WorkflowChooser`, the revert arrow, "· not feature",
   the footer divider and the `new-task-workflow-<name>` identifiers are gone.
   What it taught:
   - `@State` does not change in an unhosted ViewInspector view, so the
     sentence, its menus and the step line are a leaf view, `NewTaskWorkflow`,
     over bindings to the sheet's Wave and workflow. The test taps a menu
     item, then reads the view again.
   - `defaultPhrase` lives on `NewTaskWorkflow` and now also answers "for
     this Task only"; slice 3 moves it to the one whose-default function.
   - Not seen in the Dev app: the borderless menus' face and baseline beside
     the sentence, the 540 pt sheet's height without the divider.
   - Checks: `scripts/test_desktop.sh --filter NewTaskTests` passed (4 tests).
3. **`EmptyState` and the drawn states. Built** (d9a69d87a5); the design
   sections above say what stands. What it taught:
   - The wire cannot tell a Wave's `graph:` from the repository's: both are
     `ConfigSource::Repository`, and a Task with its own Graph sends no
     source.
   - `RepositoryWorkspaceContent` with a retained workspace and an error
     keeps its inline text and Retry beside the content.
   - Not seen in the Dev app: any of these states, the borderless "Start
     with…" beside the accent Start, compact tiles. qa or demo looks.
   - Checks: `scripts/test_desktop.sh --filter 'EmptyStateTests|NewTaskTests'`
     passed (8 tests); so did `unstartedTaskOffersStart`,
     `workflowReadingStates`, `workStates`,
     `finishedOwnedTaskWithoutHistoryShowsRecord`, `reopenDuringResolution`
     and `TypographyDisciplineTests`. `documented_commands::` not run after
     the one-word `docs/surfaces.md` edit; gate runs it.
4. **The remaining states. Built.** No `ContentUnavailableView` is left in
   `swift/LoopflowMac`, and `TypographyDisciplineTests` fails on a new one.
   List empties read as one sentence (`.emptyLine()`), failures beside
   content as `FailureLine`; a compact `EmptyState` with no button keeps its
   title, truncated. `workflowChoice`, `workflowSelection` and `scopeName`
   are at the end of `WorkModel.swift`. What it taught:
   - New Task could join only `WaveTiles`' empty line; the Project and Wave
     lists have no `WorkModel` (questions 20).
   - `SessionsView`'s failed open had no re-read to call; its `.task` body
     became `readMachine()` so Retry exists (questions 22).
   - A `Menu` draws plain text only, so "Workflows unavailable: …" keeps the
     menu's face (questions 23).
   - No "We do these now" pointer named code this change fixed.
   - Not seen in the Dev app: any of these, in either theme. qa or demo
     looks, then deletes the two prototype sections.
   - Checks: `scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|RemoveWaveSheetTests|PullRequestTests'`
     passed (14 tests). `TaskGraphProofTests` ("No steps."),
     `TaskHistoryProofTests` and `documented_commands::` (a `docs/surfaces.md`
     row) were not run; gate runs them.

## Docs written ahead of the build

They describe the behaviour above; implement reads them as the wording to
match.

- `docs/surfaces.md`: "New Task", "A Task that has not started", "When
  nothing is there, or something failed" (replaces "File without starting").
- `docs/orchestration.md`: `lf graph list --json`'s `workflow` under
  "Describe a Graph".
- `docs/work.md`: `graph.steps[].interactive` covers a stop inside the step.

