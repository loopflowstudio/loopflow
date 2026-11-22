# New Task and every empty or failed state match the prototype

Task 923677. Reference: `DESIGN.md` ("The language", "What we avoid") and the
"New Task" and "When nothing is there, and when something fails" sections of
`design/prototypes/desktop-more.html` (open it; it is live). Both are on main
since #2016; this builds on main.

## Outcome

Someone pressing ⌘N in Loopflow for Mac sees the prototype's sheet: "What do
you want?" in 34 pt serif, their request at 17 pt, "In loopflow, running
feature, loopflow's default" with two ordinary menus, the workflow's steps as
the same line a Task shows later, "You take part at explore, human-review and
verify." under it, and one accent button reading "Start ⌘↩".

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
moves to `Design/StateView.swift`, renamed `StateView`, and its full form is
rewritten to the prototype's `.app.state` (`desktop.css:223-228`). No second
type is added. Passed over: a new component beside `PaneEmptyState` (two of
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

**The "You take part at …" sentence exists today only as an accessibility
label** built in Swift (`NewTaskSheet.swift:237`). It joins the wire next to
the steps so one place writes it, as `TaskGraph.sentence` does.

**`StartFlowItems` (`BacklogView.swift:1228`) is already the workflow list as
menu items**, used by the Task bar's "Start with…". The sheet's searchable
popover `WorkflowChooser` is a second way to choose a workflow.

**"Open the repository conversation" is a real destination**
(`Destination.repository`, `WorkDestination.swift:8`), New Task is
`model.navigation.palette = .newTask`, and "Read the plan again" is
`model.rescanWork()` (`WorkModel.swift:1648`). ⌘K is bound to the navigator
(`SessionsView.swift:43`).

## Source of truth

- Workflow steps and the take-part sentence: Rust, on `GraphCatalogEntry`.
- Layout and wording of the sheet and the six drawn states: the prototype.
  States it does not draw follow the same form with wording from `DESIGN.md`'s
  Text rules (sentence case, digits, no middle dot, no em dash).

## Design

### Wire: `GraphCatalogEntry.workflow`

```rust
/// A Graph as the workflow line shows it before any Task stands on it.
pub struct WorkflowOutline {
    pub steps: Vec<WorkflowStep>,
    /// "You take part at explore, human-review and verify." or "Runs on its own."
    pub sentence: String,
}
// GraphCatalogEntry
/// Null exactly when `compiled` is.
#[serde(deserialize_with = "Option::deserialize")]
pub workflow: Option<WorkflowOutline>,
```

`workflow()` becomes a function over `&[ConcreteStep]` that both
`CapturedGraph` and `load_catalog_entry` call. The sentence names the
interactive entries of `steps`, so the line and the sentence cannot disagree.
Swift mirrors it with `decode(WorkflowOutline?.self, forKey:)`;
`tests/fixtures/dto/graph_catalog.json` and both languages' fixture tests gain
the field. `lf graph list --json` carries it; the text listing is unchanged.

### `WorkflowLine` for a definition

`WorkflowLine` gains a second way in: an outline with no position (every step
reads as ahead, none strong). The sheet shows the steps wrapping on their own
row and the sentence on the row below in `textSecondary`; the Graph's name is
not repeated because the menu above already says it. The Task bar's form is
unchanged. `WorkflowCatalogInspector` (`WorkPalette.swift:429`) uses the same.

### `StateView`

```swift
StateView(title: String, sentence: String? = nil, machine: String? = nil) { actions }
```

- Leading-aligned, vertically centred in its pane, padding `Spacing.xxxl`,
  gap `Spacing.sm`.
- Title: `.serif(.title)`, `palette.emphasisInk`.
- Sentence: `Typography.text`, at most about 52 characters wide.
- `machine`: the machine's message, `Typography.mono`,
  `palette.textSecondary`, selectable, wraps anywhere. A state with `machine`
  is a failure.
- Actions: `LoopflowButtonStyle`, at most one `.accent`, a shortcut inside
  its button's label ("New Task ⌘N").
- Compact (dashboard slots, `\.compactPanes`): one line, the title in
  `Typography.text` and the action; when that does not fit, the action alone
  with the title as its help. **Choice made for review:** the `systemImage`
  parameter goes entirely, so a compact slot no longer falls back to an icon.

### New Task sheet

| Part | Now | After |
|------|-----|-------|
| Question | `.serif(.title)` | `.serif(.display)` |
| Request field | `Typography.text`, radius `pane`, padding 12 | `Typography.lede`, radius `control`, `Spacing.md` |
| Sentence | "In X, running Y · X's default", bold labels with a meta chevron, popover | "In [menu], running [menu], X's default" at `Typography.text`; two `Menu`s with the standard indicator; other than the default reads ", for this Task only" |
| Workflow | `WorkflowSteps` circles | `WorkflowLine` from the entry's outline, then its sentence |
| Start | capsule, "Start  ⌘↩" | `LoopflowButtonStyle(.accent)`, "Start ⌘↩" |
| Cancel, File | plain text; neutral | both `LoopflowButtonStyle()` |
| "Not filed · reason" | warning text | "Not filed." then the reason in mono |
| Paddings 22/14/12/10 | literals | `Spacing` steps |

**Choices made for review:**
- The workflow menu is `StartFlowItems`; `WorkflowChooser` goes, and with it
  the search field and each workflow's summary line. The brief asks for
  "ordinary menus"; passed over: keeping the popover behind a menu-shaped
  button.
- The revert arrow goes; choosing the default in the menu reverts.
- File stays (the prototype draws only Cancel and Start, but filing without
  starting is shipped behaviour from #1993 and the brief does not remove it).

`StartingTaskView` (the request after the sheet closes) follows the
prototype's "A New Task that failed to start": the request at
`Typography.lede`, "Not started." strong in `palette.failure`, the error in
mono, "Try again" accent and "Discard".

### The states

Drawn in the prototype (wording is the prototype's):

| Site | Title | Sentence | Buttons |
|------|-------|----------|---------|
| `WorkSurfaceView:43`, `RoadmapView:144` with a repository | No planned Work in this repository | Say what you want built, or talk it through first. | New Task ⌘N (accent), Open the repository conversation |
| `WorkSurfaceView:52`, `SessionsView:1580` | Choose Work | Pick a Task or a Wave in the sidebar, or press ⌘K to find one in any repository. | none |
| `WorkSurfaceView:215` | Selected Work is unavailable | The latest plan does not include it. Its workspace and conversations are kept. | Read the plan again; "Open its conversations" only if the selection's workspace is still retained and can be opened |
| `RepositoryWorkspaceView:13`, `:110` | This repository did not open | error in mono | Retry (accent); "Open another repository…" where the view can reach the app's open panel |
| `StartingTaskView` | as above | | Try again, Discard |
| Unstarted Task's conversation pane (`WorkspacePanesView:93`, "Conversation not open") | the Task's title | Not started. It will run `feature`, the Wave's default, and you take part at explore, human-review and verify. | Start (accent), Choose workflow (`StartFlowItems`) |

Not drawn; same form, sentence case, next step only where the app has one:

| Site | After |
|------|-------|
| `RoadmapView:144` without a repository | "No planned Work yet", no button |
| `RoadmapView:165` | "Nothing needs action" / "No live or stopped work across these Waves." / Show roadmap (the lens switch) |
| `DashboardView:127` | "This dashboard is no longer available" / Show working Tasks |
| `DashboardView:252` | "Work unavailable", compact |
| `SessionsView:1440` | "This repository did not open", `machineError` in mono, Retry only if a re-read exists there |
| `WorkActivityView:86` | "No activity in this window" / "Work creation, runs, PRs and steers appear here." |
| `TaskFilesView:420` | "Select a file" |
| `TaskFilesView:425` | "Comparison unavailable", reason in mono, Retry through the store's existing retry if it covers changes |
| `TaskFilesView:435` | the snapshot state's title |
| `OpenSourceLicenses:51` | title and sentence as they are; nothing to press |
| `WorkspacePanesView` (6 `PaneEmptyState` sites) | same titles and actions, no icon |

Plain `Text` empties stay one line (one serif thing per screen): sentence
case, `Typography.text`, `palette.textSecondary`, full stop, and a button
beside the sentence where the app has the action.

- Gains a button: "No Tasks in this Project.", "No current Tasks",
  "No planned or started Tasks", "No Projects in this Wave." (New Task where
  the scope is known); "No Graph has run." (Start, under the bar's existing
  refusal rule); "No recorded work." (its existing Refresh, set beside it).
- Wording only: "No pull request yet · changes below are local"
  (`PullRequestStore.swift:136`) loses its middle dot, as do
  "Project unavailable · …", "Workflows unavailable · …"; "No comments.",
  "No brief yet.", "No description recorded.", "No Session history recorded
  for this Task.", "No matching files", "No matches", "No files to show",
  "No steps", "Empty comment", "Task is unavailable", "Session location
  unavailable", "Task status unavailable." get the face, colour and full stop
  where they differ.
- Inline failures ("Latest read failed: …", `TaskFilesView:297`,
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

## What goes

- `WorkflowSteps` and `WorkflowChooser` (`NewTaskSheet.swift`).
- `WorkflowDefinition` and `WorkflowCatalogEntry.workflow` as derived in
  Swift (`Graph.swift:253-323`); `WorkflowCatalogEntry` itself if the outline
  on `GraphCatalogEntry` leaves it nothing to add. The
  `DesktopHeadlessTests.swift:542` case that builds a `WorkflowDefinition`
  goes with it.
- `PaneEmptyState`'s name, its `systemImage` and icon-only compact fallback.
- Every `ContentUnavailableView` in `swift/LoopflowMac`.
- `new-task-default`'s "· loopflow's default" assertion in `NewTaskTests`
  (rewritten for the comma), and the `new-task-workflow-<name>` identifiers.
- In `DESIGN.md`: the "two workflow renderers" pointer under "Two of the same
  thing" (the Create Wave pointer stays); "Tokens today" gains `StateView` in
  the component list, the catalog's `workflow` beside `TaskGraph.steps`, and
  the `display`, `title` and `lede` rows name the sheet and state titles.

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
scripts/test_desktop.sh --filter StateViewTests
scripts/test_desktop.sh --filter TypographyDisciplineTests
lf graph list --json | jq '.[] | select(.name=="code") | .workflow'
```

The last prints `steps` plan, build, ship-review, deliver, verify with
`interactive` true on ship-review and verify, and
`"sentence": "You take part at ship-review and verify."`

View tests (ViewInspector, headless):
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

1. **The wire and the sheet's step line.** Rust `WorkflowOutline` on the
   catalog entry, fixture, Swift decode, `WorkflowLine` drawing an outline,
   the sheet showing it with the sentence; `WorkflowSteps`,
   `WorkflowDefinition` and the inspector's use go. This is the slice that
   could prove the plan wrong: it shows whether one `WorkflowLine` reads well
   both in a 34 pt bar and wrapped in a 540 pt sheet, and whether the catalog
   steps equal a started Task's for `code` and `feature` (a Rust test asserts
   the two agree for one composed Graph).
2. **The rest of the sheet.** Ramp steps, the two menus, the accent Start,
   `Spacing` steps, the "Not filed." form, `StartingTaskView`;
   `WorkflowChooser` goes. Sheet view test.
3. **`StateView` and the drawn states.** Rewrite `PaneEmptyState` into
   `Design/StateView.swift`, move the 7 pane sites, then the six prototype
   states. The empty and failed view tests.
4. **Every remaining `ContentUnavailableView`, the plain `Text` empties and
   inline failures**, then the source scan and `DESIGN.md`.
