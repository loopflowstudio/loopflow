# New Task and every empty or failed state match the prototype

Task 923677. Reference: `DESIGN.md` ("The language", "What we avoid") and the
"New Task" and "When nothing is there, and when something fails" sections of
`design/prototypes/desktop-more.html`. This change deleted those two sections
once their screens shipped; `git show fd90d7e6ad:design/prototypes/desktop-more.html`
has them, and `docs/surfaces.md` holds the wording now.

Slices 1 to 7 are built and were looked at in the Dev app
(`scratch/qa.html`, four runs). **Next:** slice 8, the diff stream's two
failures and one `Failure` value; it is the last. Then the gate checks and
one look at "Not started." on an unlocked display (questions 48).
A Task started from New Task opens beside "Files did not open" (Task ea8659,
outside this change); a demo meets it on the first Start.

## Outcome

Someone pressing ⌘N in Loopflow for Mac sees the prototype's sheet: "What do
you want?" in 34 pt serif, their request at 17 pt, "In loopflow, running
feature, loopflow's default" with two ordinary menus, the workflow's steps as
the same line a Task shows later, "You take part at explore, ship-review and
verify." under it, and one accent button reading "Start ⌘↩". (The prototype
says "human-review"; step names are whatever Rust reports.)

A Task filed and not yet started says the same thing in the same words: its
bar draws that step line, and its pane is titled "Not started" and reads "It
will run feature, Product's default. You take part at explore, ship-review
and verify." with Start and "Start with…". (The prototype says "the Wave's
default"; the pane names the scope as the sheet does: "Product's default" is
the default of the scope whose new Tasks take it up, true whichever file set
it.)

A repository with no Waves, no Tasks and no conversation open shows one
thing: "No planned Work in this repository", "Say what you want built, or
talk it through first.", "New Task ⌘N" and "Open the repository
conversation". Either button leads to the ordinary repository page. The
state stands only while no conversation is open, so it is what the Dev app
shows on a fresh repository and what the released app shows once the
conversation is closed (see "When the empty repository's state shows").

Anywhere the app has nothing to show or something failed, the pane is
left-aligned: a 26 pt serif title saying what happened, one sentence, then
buttons for what to do next. No grey icon over a centred title anywhere in
`swift/LoopflowMac`. A failure names what failed, shows the machine's message
in mono, and keeps Retry.

## Demo

`uv run python scripts/loopflow-dev.py run`, then:

1. ⌘N. Type a request, switch the workflow menu from the default to another:
   the step line and the "You take part at …" sentence change with it.
2. Open an empty repository (a fresh `git init` directory): "No planned Work
   in this repository", with "New Task ⌘N" and "Open the repository
   conversation" to press. The second brings the page with its terminal.
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

**Choice made for review:** the scope's name stands and Rust sends no layer.
"Product's default" says whose new Tasks take the workflow up, which holds
for a Wave's `graph:` and the repository's alike.

### `EmptyState`

Built: `Design/EmptyState.swift`,
`EmptyState(title:sentence:machine:) { actions }`. Leading, vertically
centred, serif title, sentence about 52 characters wide, the machine's
message in mono and selectable, `LoopflowButtonStyle` buttons. Compact
(`\.compactPanes`): the title and the buttons on one line, else the buttons
alone with the sentence or message as help. No `systemImage`.

A compact state with no button keeps its title, cut short where the slot
ends. Beside it in the same file: `.emptyLine()` for a list with nothing in
it, and `FailureLine` (what failed, strong in the failure ink, the message in
mono, the way to retry) for a failure beside content that stays. New Task's
"Not filed." and "Not started." are `FailureLine`s too (questions 43).

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
(`WorkSurfaceView.swift`; the first stands in for an empty repository's page
in `SessionsView`'s workspace panel; `RoadmapView` draws its own "No planned
Work yet" title),
the missing selection, `RepositoryDidNotOpen` (`RepositoryWorkspaceView.swift:98`),
`TaskStartOffer` (`TaskWorkView.swift:236`) and the pane sites in
`WorkspacePanesView.swift`. `docs/surfaces.md` holds their wording.

**Choices made for review:**
- The second button reads "Start with…", not the prototype's "Choose
  workflow": picking an item starts the Task, and "Start with…" is what the
  "…" menu and the backlog row call the same items. Passed over: a "Choose
  workflow" that stores without starting (a new action).
- An unstarted Task's full screen has Start twice, neutral in the bar and
  accent in the pane. The bar's stays because it is the one left when the
  conversation pane is collapsed or another pane is zoomed. There is no
  third: the Graph log's "No Graph has run." has no button.
- The unstarted pane's title is "Not started" in a pane and in a tile, not
  the Task's name as the prototype draws it alone: the brief beside the pane
  sets the name in serif already. Passed over: the name twice.
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

- Gains a button: "No planned or started Tasks." (New Task); "No recorded
  work." (its Refresh, set beside it). The backlog's empty keeps its four
  steps and takes this form, neutral button beside the line. "No Tasks in this
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
  `RemoveWaveSheet:74`, the "planning unavailable" notice on a retained
  Task's page): what failed in sans, the machine's message in mono, Retry
  kept.

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
"No workflow selected", every `ContentUnavailableView` in
`swift/LoopflowMac`, the backlog's own bold empty form, `TaskStartOffer`'s
title branch, the Graph log's Start, "Open the conversation here.",
`WorkSurfaceView`'s `NoPlannedWork` branch and the two prototype sections
with their script (their CSS stays in `desktop.css`, which `desktop.html`
shares).

Must survive: File without starting, ⌘↩ and Esc, the sheet opening on the
sidebar's Wave, the default following the scope, the request kept when
filing is refused, every accessibility identifier tests and `lf` Desktop
actions use other than the two named above, compact dashboard slots staying
one line.

## When the empty repository's state shows

**Choice made for review:** the state stands while no conversation is
attached, opening or failed, in every build, and nothing holds a
repository's conversation back. The released app opens that conversation on
its own (`SessionsWorkspace.opensConversationAutomatically`), so a fresh
`git init` directory goes straight to its page there, with a terminal and
"Nothing is waiting in this repository." beside New, and reads "No planned
Work in this repository" once the conversation is closed; the Dev app opens
nothing until a visit and arrives on the state. Qa saw both (questions 39,
46). The state's second button is "Open the repository conversation", so the
prototype draws a repository whose conversation is not open; a repository
with a live conversation has something there. Passed over: holding the
conversation back until the button is pressed, which changes when a
repository's agent starts and belongs to whoever owns that rule
(questions 49).

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
- Unstarted Task: its pane shows "Not started", the outline's sentence and a
  Start that calls `startTaskFromInput`; its bar shows the step names.
- Sheet: "What do you want?" found; choosing another workflow changes the
  step names and the take-part sentence; the Start button's label holds
  "⌘↩"; Start and File disabled on an empty request.
- Empty: a repository with no Work and no conversation shows the title, the
  sentence, a New Task button that sets `navigation.palette == .newTask` and
  a second button that opens the repository's conversation.
- Failed: `RepositoryWorkspaceContent` with an error shows the title, the
  error text, and a Retry that calls `retry`.
- Source scan beside `TypographyDisciplineTests`: no `ContentUnavailableView`
  under `swift/LoopflowMac`.

- One failure beside content with a way to retry
  (`ProjectReadinessView`): what failed, the message, and a Retry that calls
  `retry`.

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
   - Realign found one failure the slice's list missed: a retained Task
     whose planning is unavailable drew a tinted banner with a warning icon
     and "Retained Task · planning unavailable". It is a `FailureLine` now
     and `evidenceBanner` is gone (questions 26). No view test covers it.
   - Checks: `scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|RemoveWaveSheetTests|PullRequestTests'`
     passed (14 tests); after the banner change
     `--filter 'EmptyStateTests|TypographyDisciplineTests'` passed (8 tests).
     `TaskGraphProofTests` ("No steps."), `TaskHistoryProofTests` and
     `documented_commands::` (a `docs/surfaces.md` row) were not run; gate
     runs them.

5. **The empty repository, and what the Dev app showed. Built.** Swift and
   docs only. `WorkModel.repositoryIsEmpty(_:)` is the one test for "a
   repository with nothing in it"; `SessionsView`'s workspace panel shows
   `NoPlannedWork` in place of `WaveViewLayout` while it holds, and the
   second button runs `model.openConversation(in:store:)`. The backlog's
   empty is `.emptyLine()` with a full stop beside a neutral button;
   `TaskStartOffer` is titled "Not started" at both sizes; the Start beside
   "No Graph has run." and "Open the conversation here." are gone, and so
   are the prototype's two sections with their script. What it taught:
   - **The state shows only where a conversation waits for a person.** A
     repository's conversation opens on its own in the released app
     (`SessionsWorkspace.opensConversationAutomatically` is true for every
     non-Task workspace, and `conversationIsOpening` answers true as soon as
     the Machine is known). There a fresh `git init` directory goes straight
     to its page with a terminal and "Nothing is waiting in this
     repository." beside New, and reads "No planned Work in this repository"
     only after its conversation is closed. The Dev app
     (`opensUnvisitedWork` false for a `.dev` bundle) opens nothing until a
     visit, which is why qa saw "Conversation not open" and why the demo
     step reads the state there. Questions 39.
   - `WorkSurfaceView` no longer draws `NoPlannedWork`: with no selection the
     repository's page takes that place, and a stale selection in an empty
     plan reads as its own unavailable state. `NoPlannedWork` is the repository's form only;
     `RoadmapView` draws a bare `EmptyState` title.
   - `BacklogList.empty.title` also feeds `lf desktop capture -p backlog`,
     so the capture's empty line gained the full stop.
   - Not seen in the Dev app: the state in a repository's window, the
     conversation opening from its button, the backlog's one-line empty,
     either theme. qa looks.
   - Checks: `scripts/test_desktop.sh --filter 'EmptyStateTests|unstartedTaskOffersStart|workStates|BacklogViewTests|TypographyDisciplineTests'`
     passed (21 tests in 5 suites). `documented_commands::` after the
     `docs/surfaces.md` and prototype edits was not run; gate runs it.

6. **Every failure beside content is a `FailureLine`. Built.** Swift only.
   The twelve listed sites and one found beside them
   (`Views/WaveDetailPane.swift:248`, "Tasks did not load.") draw
   `FailureLine("<what failed>.", machine:)` with the button and
   accessibility identifier each had. The sidebar's three one-sentence
   empties end in a full stop ("No Tasks in this Dashboard." with them) and
   the button reads "Show working Tasks".
   - `ProjectReadinessView` carried two things in one string. A transport or
     activation error is the failure ("Project not prepared.", the message,
     Retry inside the line). "No Project selected yet." and the other
     readiness sentences say where the Project stands; nothing failed, so
     they stay as text with Retry under them (questions 53).
   - Wording that differs from the table, after reading the sites: the Files
     pane's `store.error` is a failed read or a failed file watch, so it
     reads "Latest read failed." as Task comments do, and the recovered
     resource reads "Current work not read." (questions 54).
   - Still bare, as planned: `TaskHistoryFilter.swift:78`,
     `TaskFilesView.swift:347`/`:356`, `WorkNavigator.swift:475`,
     `WavesView.swift:290`, `MobileSettings`, `TelemetryDashboardView`, and
     the metric reason at `WaveDetailPane.swift:459`.
   - None of these sites was reached on screen. qa or demo looks at
     whichever it can reach; a `FailureLine` is one `HStack`, so a long
     message in the 380 pt bind sheet is the one to look at first.
   - Checks: `scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|RemoveWaveSheetTests|RenameWaveSheetTests|WorkNavigationTests|ProjectActivationTests|emptyListStates'`
     passed at 3c4b0d4025, after the last edit (70 tests in 7 suites).

7. **The last five failures drawn another way. Built.** Swift only; no
   new type beyond the backlog's notice saying which it is. The third
   as-built concept review read every `Text(error)`, `palette.warning`,
   `palette.failure` and `exclamationmark` under `swift/LoopflowMac`; this
   list is closed, and nothing is added to it while building.
   - `Views/RoadmapView.swift:273`: a Wave card whose Tasks did not load is
     a warning triangle and the bare reason. It becomes
     `FailureLine("Tasks did not load.", machine: reason)`, the line
     `WaveDetailPane.swift:248` draws for the same `roadmap.tasks`.
   - `Views/RoadmapView.swift:283`: "LOO-1: reason · recovery" becomes
     `FailureLine("<identifier> is unavailable.", machine: task.reason)`
     with `task.recovery` as text in its trailing slot.
   - `Views/BacklogView.swift:465`, `:530`, `:551`: "Order not saved · …",
     "Brief not saved · …", "Brief not restored · …" share
     `BacklogState.notice: String?` with "Filed fc6ad0". The notice becomes
     one value that is either what was done or what failed with the
     Machine's message, and `notices` (`:737`) draws the second as
     `FailureLine("Brief not saved.", machine:)` under the same
     `backlog-notice` identifier. "Order not saved." keeps the Task's
     identifier in its `what`. Every other `backlogNotice` caller passes
     what was done, unchanged.
   - `Views/DashboardView.swift:99`:
     `FailureLine("Some Work did not load.", machine: first or "first (+n more)")`,
     one line, `dashboard-unavailable` and the help listing every reason kept.
   - `Views/TaskChangesView.swift:194`, `:754`: `hiddenNotice` holds the
     error alone and draws as
     `FailureLine("Diff settings did not load.", machine:)`.
   - Stay as they are, because nothing failed: "Partial: searching the N
     loaded", "Older Tasks are truncated.", "Brief changed since; not
     undone", "The brief changed since you last read it.", the sidebar's
     triangle beside a Wave's name (`WorkNavigator.swift:214`), and the
     sites slice 6 lists as still bare.
   - A `FailureLine` that is a narrow container's whole content sets its
     smallest size (questions 56). These five sit beside rows that set the
     width; the line gets no floor of its own.
   - Checks: one view test for the backlog (a failed brief save shows "Brief
     not saved." and the message; a filed Task's notice still reads as
     text), then
     `scripts/test_desktop.sh --filter 'BacklogViewTests|EmptyStateTests|TypographyDisciplineTests|WorkNavigationTests'`.
     Dev app: none of the five is reachable without breaking the Machine;
     qa looks at whichever it can.
   - Check run after the last edit: `scripts/test_desktop.sh --filter 'BacklogViewTests|EmptyStateTests|TypographyDisciplineTests|WorkNavigationTests|NewTaskTests|TaskFilesTests'`
     passed (124 tests in 6 suites), the new backlog test among them.
   - As built: the backlog's notice is `BacklogNotice` (`.did` or
     `.failed(what, machine:)`); a refused rank reads "A-1's order not
     saved." The Changes sidebar's notice also carried a failed save
     ("Hidden paths are not saved: …") through the same draw site, so it
     is a `HiddenPathsFailure` (what, machine) and the save reads "Hidden
     paths were not saved." (questions 63). The sidebar is 236 pt wide: its
     line is the one to look at first for wrapping: `FailureLine` sets
     `what` and the message side by side, where the old text ran full width.
   - `BacklogNotice.failed` and `HiddenPathsFailure` hold the same pair
     `FailureLine` takes (what, machine). The fourth compress left them: a
     shared value swaps one five-line type for another plus a second
     `FailureLine` initializer, and the other 30 sites pass the pair inline.
     That pass cut only `WorkflowOutline`'s public initializer (a test was
     its one caller; `@testable` reaches the memberwise one);
     `scripts/test_desktop.sh --filter 'WorkflowLineTests|NewTaskTests'`
     passed after it (11 tests in 2 suites).

8. **The diff stream's two failures, and one value for a kept failure.**
   Swift only. The fourth as-built concept review found the one listed site
   slice 6 passed by (`ReviewStreamView:69` in the inline failures above)
   and what sits beside it. Closed list; the sweep ends here.
   - `Design/EmptyState.swift`: `struct Failure: Equatable { let what:
     String; let machine: String }` and `FailureLine(_ failure:)` with and
     without actions. `BacklogNotice.failed` carries a `Failure`;
     `HiddenPathsFailure` goes and `hiddenNotice` is a `Failure?`. The
     other sites keep passing the pair inline.
   - `Views/ReviewStreamView.swift:64`: a file whose read failed draws
     `FailureLine("File not read.", machine: reason) { Retry }` with the
     note's padding and the Retry it has (`store.retryFile`), as a
     `LoopflowButtonStyle` button. `FileNote.failed` holds the reason;
     `FileNote.text`'s "Could not be read: …" goes. `.cut` stays a
     `ReviewNotice`.
   - `Views/ReviewStreamView.swift:24`: `store.gapError` is a `Failure?`,
     "Skipped lines of <path> did not load." with the message, drawn as a
     `FailureLine` with the note's padding
     (`TaskFilesTests.swift:1132` reads it as nil; unchanged).
   - `Views/WaveDetailPane.swift:179`: `closed.error` holds the message
     alone, so the line stops reading "Completed Tasks did not load.
     Completed Tasks unavailable: …".
   - Stay, and are not looked at again: window `Notice`s ("Could not change
     pane: …"), an icon whose help holds the reason
     (`WorkNavigator.swift:214`, `WorkspacePanesView.swift:466`), a file's
     save message, the tour's status, form checks, the Files pane's
     "Latest read failed." over "Live file observation unavailable: …"
     (questions 54), and `ConversationPlaceholder.namesFailure` repeating
     the placeholder's two conditions.
   - Checks: one view test (a stream file with `.failed` shows "File not
     read.", the reason, and a Retry), then
     `scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|BacklogViewTests|TaskFilesTests'`.
     Dev app: the failed file shows in the Diff tab with the server
     stopped (`scratch/qa.html`, fourth run, item 2); qa looks.

Outside this change, on the page for whoever wants it: a stopped Graph's
reason is one string on the wire, so the Machine's part of "kickoff:
execution failed: …" cannot be set apart in mono without Rust sending the
two parts; its own Task. Also a Start the Machine
refused goes to `CreationRecovery`'s notice (a request UUID, cut off at the
window's edge) and not to `StartingTaskView`'s "Not started." with Try
again, which shows only when the request never reached the Machine. Telling
a refusal from a lost answer is a change to creation recovery, its own Task. A new Task's Files pane pointing at the repository
(questions 45) is `SessionsWorkspace.files(for:)`, untouched here; its own
Task.
Seen at qa and left: the chevron between a menu's name and its comma in the
sheet; "No Graph has run." under "Waiting for you at explore.".
`lf desktop capture -p backlog` still prints "Wave planning unavailable · …"
(`WorkModel.swift:1035`); it is capture text, not a drawn state.

Every slice was looked at in the Dev app at qa (`scratch/qa.html`): the
sheet, the unstarted Task, the failed repository, the empty repository and
its two buttons, "Not filed." and "Not started." with the Machine away, and a
Wave's default read as designed. The second run fixed one state the slices
missed (a Files pane that could not open is "Files did not open", questions
44) and left three findings on the page: a new Task's Files pane pointing at
the repository (questions 45), about twenty bare failure texts beside content
(questions 47; slice 6 takes them), and the released build never arriving
on the empty state (questions 46; settled above). Left for later steps: the
gate checks above.
`scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|NewTaskTests'`
passed after the fix (12 tests).

## Docs written ahead of the build

They describe the behaviour above; implement reads them as the wording to
match.

- `docs/surfaces.md`: "New Task", "A Task that has not started", "When
  nothing is there, or something failed" (replaces "File without starting").
- `docs/orchestration.md`: `lf graph list --json`'s `workflow` under
  "Describe a Graph".
- `docs/work.md`: `graph.steps[].interactive` covers a stop inside the step.

Third qa (at 09351ba8ac): the Open Work window had grown to 480 by 1,989 pt
with slice 6's `FailureLine` as its whole content; it has a minimum width
now (questions 56). The workspace's "Layout not saved: …" became the same
`FailureLine` the Dashboard tile draws (questions 57).
`scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|NewTaskTests|WorkNavigationTests|workStates'`
passed after both (63 tests in 5 suites); qa ran it.

Fourth qa (at 406697d19c): four fixes. `FailureLine` stacks its message
under what failed when the row does not fit (questions 65); the line above a
workspace's panes stays away while a pane says the conversation failed
(66); the Changes pane's failed comparison is a `FailureLine` (67); the
Dashboard's line is at the leading edge (68). The diff stream's "Could not be read: …"
note (69) is slice 8.
`scripts/test_desktop.sh --filter 'EmptyStateTests|TypographyDisciplineTests|NewTaskTests|WorkNavigationTests|BacklogViewTests|TaskFilesTests|workStates|ProjectActivationTests|RenameWaveSheetTests|RemoveWaveSheetTests|PullRequestTests|WorkCacheTests'`
passed after them (167 tests in 12 suites); qa ran it.
