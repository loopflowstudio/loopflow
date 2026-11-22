# Wave pages, backlog, dashboards, theme chooser and the remaining Mac views follow DESIGN.md

Task 8b92c6. Builds on main at 0fa6a4e202 (#2016), which shipped the fonts, the
ramp, `TaskRow`, `StatusMark`, `WorkflowLine`, `LoopflowButtonStyle` and
`TitleBar`. This change uses them and adds no component. It adds one value,
`TaskStanding`, which is what `TaskRow` already works out inline.

Reference: `DESIGN.md` ("The language", "What we avoid") and the live
prototypes `design/prototypes/desktop.html` ("A Wave's backlog") and
`desktop-more.html` ("A dashboard of Tasks side by side", "Choose Theme").
Their numbered notes are the spec for wording and layout on those three
screens.

## Outcome

Someone opening a Wave, a dashboard, Settings or the Portfolio in Loopflow for
Mac sees the same type, spacing, button and Task row as the Task workspace,
and the same Task says the same thing about itself on each of them. Nothing on
those screens is an uppercase label, mono prose, a typed glyph, a card inside
a card or a stripe without a status.

## Done when

- A Task reads the same in the sidebar, the title bar, a dashboard tile, the
  roadmap, the Now list and a Wave's page: one status word, and one line when
  it needs someone. One function decides both.
- The Now list's headings and its rows agree: a Task is listed there when
  its row says it needs someone, and under that word.
- A Task's own page says the word its row says: the workflow header and the
  jump palette print `TaskStanding`'s word.
- A click on a Task's row opens the Task, on every Portfolio screen.
- A Wave's backlog shows the Wave's name in serif at the display step, one
  filter control, an action bar of the one button that begins with what is
  selected, a checkbox and a disclosure before each title, and ownership as a
  word.
- A dashboard tile is headed by the Task row; its workflow is the sentence;
  "Page 1 of N" is sans.
- Choose Theme has no "Mode" caption, and its sample shows the roles where
  they sit in the app.
- The Portfolio's Wave page and roadmap have no card inside a card and no
  striped warning card; a contract issue is a bordered notice.
- New Wave is one form wherever it opens.
- `swift/LoopflowMac` passes no padding, spacing or corner radius as a number,
  and the check that says so is a test.
- DESIGN.md's Desktop pointers this change fixes are gone, and "Tokens today"
  describes what ships.

## Acceptance check at gate

```
scripts/test_desktop.sh --filter DesignDisciplineTests
```

passes, and this prints nothing:

```
grep -rnE '\.padding\(([^)]*, *)?-?[0-9]|spacing: *[1-9]|[cC]ornerRadius: *[0-9]|(fixedS|s)ize: *[0-9]' swift/LoopflowMac --include='*.swift'
```

The view suites that inspect the changed screens pass:
`BacklogViewTests`, `DashboardWorkspaceTests`, `RoadmapViewTests`,
`CreateWaveSheetTests`, `ThemeWorkflowTests`, `TaskRowTests`,
`TaskStandingTests`, `WorkflowLineTests`, `WorkPaletteTests`.

Looked at in Loopflow Dev (`uv run python scripts/loopflow-dev.py run`, after
checking the Dev app is not in use and the installed `lf` is at the branch's
store frontier): the backlog, a Portfolio Wave page, a dashboard and Choose
Theme, in Loopflow Light, Loopflow Dark and one imported Ghostty theme. That
look needs a display, so it belongs to qa and review, not to build.

## What the code is today

Slices 1 to 5 are built. The acceptance grep prints nothing (seen again
after slice 4's commit, 6fe53188bb) and the Checks table has
`DesignDisciplineTests` passing. Slice 5 moved the four readers the review
of the built screens found (`scratch/concept-review.html`, at 021b28484a)
onto `RoadmapTask.need` and `TaskStanding`. Nothing is left to build; what
remains is the look on an unlocked display and the three Tasks under
"Outside this change".

- At the base, 177 lines passed a literal padding and 132 a literal non-zero
  `spacing:`, with off-scale values 1, 3, 5, 6, 7, 9, 10, 14, 15, 17, 18, 22,
  28, 30, 36, 72, 96. All are named steps now.
- SwiftUI text is on the ramp and the test holds it. Four AppKit views set a
  system font by number and no rule reads them (`ofSize:` has a capital S):
  the file editor (`TaskFileDocument.swift:133`, system mono 12), the history
  filter's counts (`TaskHistoryFilter.swift:128`, `:186`), the terminal
  search count (`TerminalSearchBar.swift:46`, 11) and the licenses text
  (`OpenSourceLicenses.swift:72`). They were there at the base. See
  question 34.
- 52 `.buttonStyle(.plain)` remain, on rows and icon buttons that draw
  themselves; no `.bordered`, `.borderedProminent` or `.link` is left.
  `.controlSize` remains on progress spinners and three checkboxes.
- `WorkSectionHeading` and `WorkDisclosureHeading` are sentence case and
  their comments say so. No `uppercased()` or tracking is left on a label.
- The changed-directory badge in the Files tree is still a typed "•"
  (`CheckoutTree.swift:159`, drawn at `TaskFilesView.swift:275` in the column
  that holds M, R and U). See question 35.
- `WaveStandupBoard.swift` no longer exists. The Portfolio window
  (`WavesView`) mounts `WaveDetailPane` and `RoadmapView`; those are the Wave
  page and the standup's successor.
- Task rows and the seven ways of saying where a Task stands are gone since
  slice 1: one `TaskRow` drawing one `TaskStanding`, and a click opens the
  Task on every Portfolio list. The backlog row is its own view; since
  slice 2 its word comes from `TaskStanding`, and its click selects for the
  action bar.
- A Project's key results are drawn once, by `WaveProjectView`, since
  slice 2.
- New Wave is one form since slice 3: `CreateWaveSheet` (`WavesView.swift`),
  a sheet in the Portfolio and a popover under the sidebar's Waves.

## Design

### Where a Task stands, said once

A person asks one thing of a Task's row: where does it stand, and does it
need me. `TaskStanding` (in `Design/`, beside `TaskRow`) is that answer: a
word, its tone, its tooltip, and an optional line. It is built in one place
from a `RoadmapTask?` and a `SessionProgramStatus`, in this order:

1. A program that reports or is available: its label, tone and help, as
   `TaskRow` prints them now. Reported Blocked keeps its own word.
2. Session readings unavailable: no word, the info mark with the reason, as
   now.
3. Otherwise the Task's condition, when it is blocked or waiting: "Blocked"
   or "Waiting" in that tone.
4. Otherwise its lifecycle, capitalized; "Done, date unavailable" when done
   with no date. An unknown condition adds no word.

The line is the program's ask when it has one, else `condition.reason` when
the condition is not clear, which is the rule the Brief already applies
(`TaskBriefView.swift:128`). A screen with no Session readings passes
`.unknown` and gets rules 3 and 4.

`TaskRow` draws a `TaskStanding` and stops choosing. It gains an initializer
from a `RoadmapTask`, for hosts that hold one; the sidebar keeps
`presentation` for a row named before planning answers. `TitleBar` prints
the same word, and the Brief's "Blocked: reason" line reads `TaskStanding`
too.

#### Whether a Task needs someone is one predicate (slice 5)

Qa found the server marks an unstarted Task whose next move is Start as
`waiting`, reason "ready", and `TaskStanding` grew a private rule that does
not count that as a need. The Now list never learned the rule: it groups by
the wire's `condition.state` (`nowGroup`, `swift/Loopflow/Models/
NowProjection.swift`), so "Waiting 9" heads nine rows that read "Unstarted".
Two deciders on one screen.

- `RoadmapTask.need` (in `WaveWorkMap.swift`, beside `awaitsStart`) returns
  the condition state when the Task needs someone and nil when it does not:
  nil for `clear`, nil for `waiting` on an unstarted Task whose recommended
  action is Start or on a finished Task, the state otherwise. It is the rule
  that was inline in `TaskStanding.init`, moved, plus the finished case the
  build found.
- `TaskStanding` and `nowSections` read `need`, so the Now list
  omits Tasks that wait only to be started; the backlog is where those are
  listed and started. `NowProjectionTests` gains that case.
- Choice made for review: the Now list gets shorter. Passed over: keeping
  every unstarted Task under "Waiting" and renaming the heading, which
  leaves the list of what needs someone drowned by the backlog.

#### The Task's own page says the row's word (slice 5)

- `TaskWorkflowHeader` (`TaskWorkView.swift`) prints
  `TaskStanding(task:status:)`'s word and tone, with the program status the
  title bar passes (`model.programStatus(task:)`). Its private
  `lifecycleLabel` ("Completed", "Canceled") is deleted, so the page says
  "Done" and "Abandoned" as every row does. The identifier
  `task-lifecycle-status` stays. `TaskConceptTests` and
  `WorkCLISubscriptionTests` move to the row's words.
- The jump palette's detail (`WorkPalette.swift:129`) prints the same word:
  "LOO-515 · Unstarted", not "Not started".
- Choice made for review: "Done" and "Abandoned" win because they are the
  lifecycle's own words, the ones `lf task status` prints. Passed over:
  teaching every row "Completed" and "Canceled".

#### A reason is said once, and does not hide the workflow (slice 5)

- A dashboard tile shows its workflow sentence unless its program asks
  something (`status.ask`), not unless the row has any line
  (`DashboardView.swift:246`). A Task parked at demo then reads "Waiting" on
  its row and "You are next needed at demo." under it, where the sentence
  was replaced by the server's "ready".
- The Brief's state line already ends with `nextMove.reason`; for a waiting
  Task the standing's line is the same string, so the page says it twice
  ("Idle, at demo. ready." then "Waiting: ready"). The Brief prints the
  standing's line only when it differs from the reason above it
  (`TaskBriefView.swift:129`).

Choice made for review: the condition's reason shows on the sidebar row as
well as in the Portfolio, so a sidebar row whose Task is blocked or waiting
gains a line it does not have today. Passed over: a row that shows the reason
only in the Portfolio, which is the same Task saying two things.

Choice made for review: the order above is decided in Swift. A program's
report reaches Desktop through Session readings and never reaches
`RoadmapTask`, so Desktop is the first place the two meet. The CLI prints no
row that merges them.

### The scale becomes a test

`TypographyDisciplineTests` becomes `DesignDisciplineTests` and gains rules
for a literal padding, a literal non-zero `spacing:` and a literal radius in
`Loopflow` and `LoopflowMac`, outside `Design/` for radius only. DESIGN.md
rule 8 names the test. Without it the count drifts back, as it did from 166
in the audit to 177 now.

Off-scale values snap to the nearest step, and a tie goes down, because the
app is dense by rule:

| Was | Becomes |
|-----|---------|
| 1, 3 | `xxs` 2 |
| 5, 6 | `xs` 4 |
| 7, 9, 10 | `sm` 8 |
| 14 | `md` 12 |
| 15, 17, 18 | `lg` 16 |
| 22 | `xl` 20 |
| 28 | `xxl` 24 |
| 30, 36 | `xxxl` 32 |
| −3, −4 | `-Spacing.xs` |

72 and 96 are indents that line a block up with a column above it (the
backlog brief under its title). They become layout: the brief sits in the
same column structure as its row, so no number says where the title starts.

No step is added. `frame(width:height:)` numbers are sizes of things (column
widths, icon boxes), not spacing; they stay, except hit targets, which use
`HitTarget`. Choice made for review: frames are left out of the test.

### Backlog (`BacklogView.swift`), per the prototype's notes 3 to 9

- **Name.** `wave-title` is `.serif(.display)` in `emphasisInk`, for a Wave
  and for the repository board. `WaveViewHeader` (backlog hidden) uses the
  same step. It is the one serif thing on the screen.
- **Filter.** `BacklogFilterPicker` stays one control: three segments at
  radius `control`, the chosen one in the theme's selection fill with
  `textStrong`, counts in tabular digits. The accent capsule goes; accent is
  for the action. The prototype's "Unstarted" menu was a guess at a control
  the app no longer has, so nothing is added for it.
- **Action bar.** Begins with "N selected" ("Nothing selected" at zero), then
  Start, Claim, Edit, Move, Abandon…, all `LoopflowButtonStyle`; Start is the
  surface's one `.accent`. "8 of 46" and the two arrows close the bar.
  `BacklogQuietButtonStyle` and the capsule in `BacklogStartButton` are
  deleted; the split Start keeps its menu as a second button of the same
  style. `startLabel` reads "Start with feature", not "Start · feature". The
  folding order under `ViewThatFits` stays.
- **Row.** Checkbox, then a chevron that shows or hides the brief, then the
  ID in mono, the title, the standing, ownership, and the "more" menu last.
  The chevron reflects `list.opened`; it adds no state. This row is not
  `TaskRow`: it is a line in a ranked list of work mostly not started, with
  the ID first and the title on one line, as the prototype draws it.
- **Standing.** A started Task in the backlog leads with `TaskStanding`'s
  word, then its step and PR, joined by commas: "Idle, at build, PR #12".
  `BacklogList.standing` keeps the step and the PR and stops falling back to
  `nextMove.reason`; the word and the reason come from `TaskStanding`.
- **Key results.** The objective's disclosure shows the Project through
  `WaveProjectView`, the one drawing of key results. The backlog's own
  check-and-cross `Label`s go.
- **Ownership.** `BacklogOwnerMark` is deleted. A claimed row says "claimed
  by you" or "claimed by Ana" in `meta`, after the Wave's name when the row
  shows one ("Growth, claimed by Ana"). An unclaimed row says nothing; the
  Available count already does. `BacklogOwner.mark` in the plain-text listing
  follows the same words.
- **Brief.** `text` at 13 with `lineSpacing(Spacing.xs)`, held to its
  readable width, aligned under the title by layout.
- **Sentences.** "Project unavailable · reason", the notice and the footer
  join with a full stop or a comma, not a middle dot. A Task ID is the only
  mono in them.

### Portfolio: Wave page and roadmap (`WaveDetailPane.swift`, `RoadmapView.swift`)

- The Wave's name on its page is `.serif(.display)`. On the roadmap, where
  several Waves share a screen, names are `lede`.
- A Wave on the roadmap is a section divided from the next by a hairline, not
  a bordered card holding filled rows. `RoadmapWaveCard` loses its border and
  fill.
- `RoadmapTaskRow`, `NowRowView` and `RoadmapTaskView` are deleted for
  `TaskRow`, built from the `RoadmapTask` each already holds and a
  `TaskStanding`. The Now row's Wave name goes in the row's context slot.
  `WorkChannelChips` goes with them: the standing's word and line carry the
  condition and the lifecycle, and the tracker's own state ("PM") was a
  third copy of done or not done. `TaskSection.label`, its color and
  `TaskConditionState.color` go with their last readers.
- A click on a Task's row opens the Task, on the roadmap, the Now list and
  the Wave page alike. `TaskActionCluster` sits beside the row on all three,
  its verbs as `LoopflowButtonStyle` buttons.
- `WaveWorkInspector` and `selectedTaskID` are deleted. Choice made for
  review: what the inspector added to the row was the provider's name and
  the checkout's path as text; both are in the Task workspace the click
  opens, and "Worktree" in the cluster opens the checkout. Passed over:
  keeping a select-then-open step on one of three lists.
- The metric table header is sentence case in `meta`, untracked. The table
  keeps `workPanel()` as the one framed surface on the page.
- Contract issues: the tinted, striped block becomes a notice with a hairline
  border at radius `row`, the count as its title in the warning role with an
  SF Symbol, each issue's summary in `text`. `MetricContractIssue` carries a
  summary and no recovery, so the next step offered is "Copy" for the
  summaries. Choice made for review; a real next step needs the wire to carry
  one.
- "Project history" in `WaveDetailPane` joins the header as a
  `LoopflowButtonStyle` button.
- `WorkSectionHeading` and `WorkDisclosureHeading` stay, with their comments
  corrected. Choice made for review: the brief says they go, but since #2016
  they are sentence-case headings, and DESIGN.md names a section "by a heading
  in sentence case". Passed over: deleting both and inlining the same five
  lines at nine call sites.

### Dashboard (`DashboardView.swift`, `DashboardSlotGrid.swift`)

- The header is the dashboard's name at `.serif(.title)` ("Working Tasks" on
  that board), the query field in mono beside it, then "Edit Slots" or
  "Default Slots" and "Edit Dashboard" as the one button.
- A tile's header is `TaskRow` (title in full, ID, status word, the ask),
  with "open in full", "more" and "hide" as icon buttons at its end.
  `TaskHeading` is deleted; `TitleBar` prints `TaskStanding`'s word, so the
  bar and the sidebar row agree about the open Task.
- Under the row, the workflow is its sentence: `WorkflowLine` gains a
  sentence-only setting the tile uses, since a tile's width is known to be
  too narrow for the steps. A row with an ask shows the ask instead.
- The pager is `Typography.text` with tabular digits and icon buttons.
- `DashboardEditor`'s title is `.serif(.title)`; its example is `meta` in a
  palette role, its buttons the one button with Save as accent.

### Choose Theme (`ThemeChooser.swift`, `ThemeEditor.swift`)

- "Choose Theme" at `.serif(.title)`. The "Mode" caption is deleted; the
  segmented control says what it is.
- The row of five colored words becomes a sample in the previewed theme: a
  title bar strip with the repository's name, a selected Work row reading
  "Blocked since 12:07", Working, Available, Blocked, Failed and Passed as
  status words on the sidebar surface, and one terminal line,
  `$ lf task status LOO-515`, in the theme's terminal colors.
- Apply is the surface's accent button.
- Choice made for review: the tile grid and the two per-side pickers stay.
  The prototype draws a plain list of names and one mode, but a tile shows a
  theme's own terminal colors before a click, and System mode assigns a light
  and a dark theme separately. Passed over: the prototype's name list.

### One New Wave form

`CreateWaveSheet`'s form becomes the only one. It shows the repository picker
when it is given more than one repository. `WavesView` presents it as a sheet
with the Portfolio's repositories; `NewWaveRow` presents it in its popover
with the window's one. `NewWaveRow`'s own fields, error line and Create
button are deleted. Field captions that repeat the placeholder ("Wave name"
over "Wave name") go; Cancel is the one button, neutral.

### Everything else in `swift/LoopflowMac`

Settings (`ConfigurationSettingsView`, `MobileSettings`,
`KeyboardShortcutsView`), `SessionsView`, `WorkActivityView`,
`TaskHistoryView`, `TaskCommentsView`, `TrackerSyncView`,
`ProjectHistoryView`, `OpenSourceLicenses`, the sheets
(`RenameWaveSheet`, `RemoveWaveSheet`, `SessionActionSheet`),
`TelemetryDashboardView`, `GraphDiagramView`, `WorkPalette`, and the Task
workspace files that still hold literals (`TaskChangesView`,
`ReviewStreamView`, `TaskFilesView`, `TaskWorkView`, `NewTaskSheet`,
`WorkNavigator`, `WorkspacePanesView`):

- Named steps for every padding, spacing and radius, by the table above.
- System button styles become `LoopflowButtonStyle`; `.secondary` and system
  colors become palette roles.
- Mono only where the text could be pasted into a terminal. Known prose in
  mono: "Page 1 of N", "builtin Graph" (`WorkPalette.swift:380`), the
  activity time (`WorkActivityView.swift:116`, `meta` with tabular digits).
  The other 66 uses are read one by one.
- Typed glyphs become SF Symbols or words: "⚠" before a changed previous
  version, the "•" unsaved-draft mark in `TaskFilesView` (three places).
- `WorkActivityView`'s `KIND · subject` loses `uppercased()` and its
  tracking: "Wave, Product".
- `WorkChip`'s `tracking(0.2)` and off-scale padding go.

### DESIGN.md

Removed under "We do these now": the Desktop pointers for uppercase labels,
mono as decoration, values outside the scale, the two Create Wave forms, and
glyphs as icons. "Tokens today" gains the discipline test's new name and
scope, and the components list stays as it is.

Kept, with its reason already written there: `WorkflowSteps`
(`NewTaskSheet.swift`) beside `WorkflowLine`. A Graph definition has no
top-level steps on the wire, so one renderer needs a Rust and DTO change.
Choice made for review: that is its own Task, so one Desktop pointer
survives this change. See `scratch/questions.md`.

`design/prototypes/` stays: `desktop.html` and `desktop-more.html` also hold
New Task and the empty and failed states, which have not shipped.

## Docs written ahead of the code

`docs/surfaces.md` ("Loopflow Desktop", "The backlog", "Start picks a
workflow") and `swift/README.md` (the Task row, New Wave, Choose Theme's
sample, the Portfolio's rows and contract notice, "Dashboards") describe the
intended screens. Build matches them or changes them in the same slice.
`DESIGN.md` is untouched until slice 4.

Wording the docs settled, not in the prototype:

- Several Tasks on one workflow: `Start 2 with feature`. Mixed workflows:
  `Start 3: feature 2, code 1`.
- A claimed row in another Wave: "Infrastructure, claimed by Ana".
- The program's status words stay **Busy**, **Available**, **Blocked**,
  **Done**; the prototype's "Working" is the theme role's name, not a row's
  word.
- The backlog's single open brief is closed from its chevron.

## Absent and error states

- Backlog empty: unchanged words and step, the title in `textStrong`, the
  step as the accent button.
- A tile whose Task has no Graph yet shows the row alone; a tile with no Task
  ("Work unavailable") keeps its placeholder.
- A theme missing a role fails to decode before the sample draws, as today.
- New Wave with one repository shows no picker; with none, Create is off.

## Exclusions

- The shared empty or failed state. DESIGN.md describes one component and
  none exists; the 14 `ContentUnavailableView` sites and `PaneEmptyState`
  stay until a Task builds it. This change adds no component.
- New Task's `WorkflowSteps`, as above.
- Middle dots in strings built outside views (`PullRequestStore`,
  `SessionProgramStatus`, `ReviewDiff`): one voice per event means changing
  them with their tests and their CLI twins, not in a styling pass.
- iPhone and the website.
- No Rust, wire or DTO change.

## What goes

`BacklogOwnerMark`, `BacklogQuietButtonStyle`, the capsule Start control,
`RoadmapTaskRow`, `NowRowView`, `RoadmapTaskView`, `WorkChannelChips`,
`WaveWorkInspector`, `TaskHeading`, `TaskRow`'s inline choice of status word,
`TaskSection.label` and the two status color extensions in
`RoadmapView.swift`, the backlog's key-result `Label`s, `NewWaveRow`'s inline
form, the striped contract-issue block,
`WaveMetricTableHeader`'s uppercase cell, `TypographyDisciplineTests` under
that name. Tests that name these (`BacklogViewTests`, `RoadmapViewTests`,
`DashboardWorkspaceTests`, `CreateWaveSheetTests`, `ThemeWorkflowTests`)
move to the surviving view and identifier.

What survives: every backlog action and shortcut, drag reorder, the folding
toolbar, undo registration, tile drag and drop and hide, dashboard slot
editing, theme import, customize, delete and the per-side assignment, Start,
Open PR, Worktree and the PR link on every Portfolio Task, the reason a Task
is blocked or waiting, the New Wave leftovers receipt, and every accessibility identifier that a test
or `lf desktop` reads unless its view is deleted.

## Slices

1. **One standing, one row, every host.** Built. `TaskStanding`
   (`Design/TaskStanding.swift`) decides the word and the line; `TaskRow`
   draws it; the sidebar, the title bar, a dashboard tile, the roadmap, the
   Now list and the Wave page all show it. `PortfolioTaskRow`
   (`RoadmapView.swift`) is the row plus `TaskActionCluster`, used by the
   three Portfolio lists. Deleted: `RoadmapTaskRow`, `NowRowView`,
   `RoadmapTaskView`, `WorkChannelChips`, `WaveWorkInspector`, `PrLink`,
   `TaskHeading`, `TaskSection.label` and both color extensions,
   `historyLabel`, `terminalLabel`. Compress took what those left
   without a caller: `WaveLens.forTask`, `TaskOwner.summary` and
   `workDestinationSelection`.

   What it taught:
   - No host needed a switch. A row opens through `TaskRow.opening(_:)`, a
     button behind the row so the ID's link stays its own target.
   - A program that reports Busy does not hide a waiting or blocked
     condition: the word is the program's, the line is the condition's
     reason. `TaskRowTests` holds this.
   - Not looked at on a display. `TaskRow` pads itself 12 pt at the sides, so
     Portfolio rows sit 12 pt inside their section headings, and a tile's
     three icon buttons sit beside a wrapping title. Both are for the Dev app
     look in qa.
   - The Now list's group headings lost their warning color with the color
     extension; the rows' words carry it.
   - Two more places say where a Task stands in other words, outside the
     seven listed above: `TaskWorkflowHeader` ("Completed", "Canceled" where
     the row says "Done", "Abandoned") and the jump palette's detail ("Not
     started", else the lifecycle word). Left; see `scratch/questions.md`.
2. **The test and the backlog.** Built. `DesignDisciplineTests` holds the
   ramp, padding, spacing and radius; `BacklogView.swift`,
   `Design/WorkStyle.swift` and the focus ring in `DesignSystem.swift` pass
   it. The backlog is rebuilt to the prototype: serif name, one segmented
   filter, an action bar of the one button that begins with "N selected",
   checkbox and chevron before each ID, ownership as words, the standing from
   `TaskStanding`, key results through `WaveProjectView`. Deleted:
   `BacklogOwnerMark`, `BacklogOwner.mark`, `BacklogQuietButtonStyle`, the
   capsule Start, the key-result `Label`s.

   What it taught:
   - The snap table held without a change. The 9 pt row padding became a
     32 pt minimum row height (`HitTarget.comfortable`) with `xs` padding,
     and the 5 pt search field a 24 pt minimum height, so neither needed a
     new step. Whether rows read cramped is for the Dev app look.
   - The 96 pt indent became `BacklogUnderTitle`: the row's own columns,
     empty, before the brief and the reason line. The brief stays a sibling
     of the row, so drag, drop and click reach the row alone.
   - A blocked or waiting Task that has not started shows its word too;
     otherwise the reason line would have no word above it.
   - Toolbar menus (Move, Start's ▾, the folded "more") use
     `.menuStyle(.button)` under `LoopflowButtonStyle`. Not seen on a
     display: whether AppKit draws the custom style on a `Menu` is for qa.
   - "Claim 3" and "Abandon 3…" lost their counts; the bar's first words say
     the count. The position reads only for one selected Task.
   - `lf desktop capture -p backlog` keeps " · " between a row's columns and
     in its heading; the words inside each column follow the screen.
   - The test's failures are the list for slices 3 and 4: 301 lines, all
     outside these three files, `swift/Loopflow/Services/Notice.swift`
     among them.
3. **The rest of the dashboard and the Portfolio, and New Wave.** Built.
   The dashboard is headed by its name in serif ("Working Tasks" on that
   board; an unsaved query has none) with the query beside it; a tile shows
   `WorkflowLine(sentenceOnly:)` under its row unless the row already prints
   an ask; the pager is `text` with tabular digits; the editor's title is
   serif and Save is its accent. The Wave page's name is serif at the display
   step with "Project history" in its header; a Wave on the roadmap is
   `RoadmapWaveSection`, divided from the next by a line; the metric header
   is sentence case in `meta`; contract issues are
   `WaveContractIssueNotice`, above the table, with Copy. `CreateWaveSheet`
   is the one New Wave form and `NewWaveRow` shows it in its popover.
   Deleted: `RoadmapWaveCard`'s border and fill, the striped block, the
   uppercase cell, `NewWaveRow`'s fields, error line and Create button.

   What it taught:
   - The snap table held again, on 47 more lines, with no new step.
   - A tile reads its workflow from `model.taskWork`, the reading its "more"
     menu already used. Whether every tile on a board has that reading
     without being opened was not seen; a tile without one shows the row
     alone.
   - The sidebar's New Wave popover is now the sheet's 420 pt form with a
     serif title, where it was two 200 pt fields. Whether that reads heavy in
     a popover is for the Dev app look.
   - Left for slice 4 in these files, since no grep line names them: the
     Portfolio's system-styled buttons and pickers (`WavesView`
     `portfolioControls`, the roadmap's lens picker), `.secondary` in
     `WorkTaskIdentity`, mono counts and "⌘N" in `WorkNavigator`, and the
     middle dots in `RoadmapView.swift:293` and `WaveDetailPane`'s metric
     detail.
4. **Choose Theme, then the sweep.** Built. Choose Theme has a serif title,
   no "Mode" caption, `ThemeSample` beside the tiles (title bar, selected
   row, five status words on the sidebar's surface, one terminal line, all
   read from the previewed theme) and Apply as the accent. Every literal
   padding, spacing and radius in `swift/LoopflowMac` and
   `swift/Loopflow/Services/Notice.swift` is a named step, by the snap table.
   The explicit system button styles are `LoopflowButtonStyle`, and so are
   the unstyled buttons in Settings, the sheets, the Portfolio controls, the
   catalog inspectors and the inline Retry and Refresh buttons. `.secondary`
   is a palette role everywhere. Mono prose, the typed "⚠" and "•", and the
   uppercase activity label are gone. DESIGN.md lost the five fixed Desktop
   pointers and "Tokens today" names the test's scope.

   What it taught:
   - The snap table needed two values it did not list: 26 became `xxl` and a
     72 pt bottom margin under a scrolling detail became `xxxl`
     (`WorkSurfaceView.swift`); that one was room to scroll, not an indent.
   - `Typography.textSize` names the 13 pt step for the one AppKit field
     that takes a number (`WorkPalette.swift`).
   - `.secondary` in the title bar was the hierarchical style the bar sets
     to `titlebarSecondaryText`; it now names that role.
   - A segmented `Picker` keeps its hidden label in the view tree, so no
     test can say "no Mode caption"; `ThemeWorkflowTests` holds the sample's
     words and that it follows the previewed theme.
   - The sample is 280 pt wide so the five words fit one line, and the
     chooser's minimum width went from 620 to 700 to keep two tile columns.
   - Not swept: about 60 middle dots in view strings (see question 28), the
     count pills (question 22), and buttons inside menus, which AppKit draws.
   - Not seen on a display: the browser's three bordered icon buttons, the
     review comment box at the `text` step where it was `.controlSize(.small)`
     with `meta`, and every snapped padding.
5. **One answer, on the remaining four readers.** Built.
   `RoadmapTask.need` (`WaveWorkMap.swift`) says whether a Task needs
   someone; `TaskStanding` and `nowSections` read it, so the Now list omits
   Tasks that wait only to be started. `TaskWorkflowHeader` and the jump
   menu print `TaskStanding`'s word. A tile's workflow sentence yields only
   to a program's ask (`DashboardTile.workflow`), and the Brief prints what
   the Task needs only when its state line has not said it
   (`TaskBrief.need`). Deleted: `lifecycleLabel`, the palette's "Not
   started", `nowGroup`.

   What it taught:
   - The server also calls an abandoned Task `waiting`, reason "ready"
     (`tests/fixtures/dto/task_concepts.json`, FIX-3), so its row and its
     page read "Waiting" over "ready". `need` is nil for `waiting` on a
     finished Task; a third case for the Rust Task under "Outside this
     change". An active Task whose worktree is missing reads "Blocked" on
     its page, where the page said "Active".
   - The header takes the row's tone as well as its word, so "Done" and
     "Abandoned" are in secondary ink, semibold, on the header's green or
     red tint and stripe, where the word itself was green or red. Whether
     that reads weak is for the Dev app look.
   - The header and the jump menu read the Task's programs, so an open Task
     whose program reports reads "Busy" or "Available" on its page, as its
     row does, where the page said "Active" or "Idle". When Session
     readings fail the header has no word, as the row has none.
   - The Brief compares the standing's line with `nextMove.reason` without
     their closing full stops. A blocked Task's reason differs from its next
     move, so "Blocked: reason" stays.
   - The tile and the Brief are each held by a test of the function that
     decides, not of the mounted view: a tile needs a workspace registry and
     a Task work reading no fixture seeds.

## Outside this change

- **One page for a Wave.** A Wave has two: its view in a repository window
  (backlog, conversation, tiles) and `WaveDetailPane` in the Portfolio
  (objective, key results, every Task with history, metrics). They share no
  header and disclose the objective two ways. One page would mean the
  Portfolio opening a Wave in its repository window, as it already opens a
  Task, and that view gaining completed Tasks and the metric table; then
  `WaveDetailPane` goes. That changes what the Portfolio window is for, so it
  is its own Task.
- **Workflow position on a list row.** `RoadmapTask` carries
  `execution.step` and not `TaskGraph.sentence`, so the backlog says "at
  build" where the Task's own page says "build is running. You are next
  needed at human-review." One sentence on both needs the planning wire to
  carry it.
- **The standing belongs on the wire.** Rust's `task_condition`
  (`rust/loopflow/src/lf/commands/waves.rs`) calls a Task `waiting` whenever
  its next move is not its own, an unstarted Task included, and gives the
  work status's word ("ready") as the reason. Desktop corrects both in
  Swift: `RoadmapTask.need` and the row's line. The repository's rule is one
  predicate, written in Rust. That Task: `task_condition` returns `clear`
  for a Task that only awaits Start, a parked Task's reason is its Graph's
  sentence, `lf wave show` and the plan board follow, and `RoadmapTask.need`
  is deleted for `condition.state`. It changes CLI output and DTO fixtures,
  so it is not a styling change.

## Checks

| Command | Result | Who runs it |
|---------|--------|-------------|
| `scripts/test_desktop.sh --skip-build --filter 'TaskStandingTests\|TaskRowTests\|RoadmapViewTests\|TaskConceptTests\|WorkNavigationTests\|TaskBriefTests\|ProjectActivationTests\|NowProjectionTests\|DashboardWorkspaceTests\|WorkModelTests\|DesktopHeadlessTests'` | passed after slice 1 | build; CI runs the rest |
| `scripts/test_desktop.sh --filter 'BacklogViewTests\|DesignDisciplineTests'` | after slice 2: `BacklogViewTests` passed (12 tests); `DesignDisciplineTests` fails on 301 lines in other files, as planned until slice 4 | build |
| `scripts/test_desktop.sh --filter 'DashboardWorkspaceTests\|RoadmapViewTests\|CreateWaveSheetTests\|WorkflowLineTests\|WorkModelTests\|WorkNavigationTests'` | passed after slice 3 (125 tests; one new assertion corrected, then `CreateWaveSheetTests` rerun, 4 tests) | build |
| `scripts/test_desktop.sh --filter 'DesignDisciplineTests\|ThemeWorkflowTests\|WorkPaletteTests\|DesktopHeadlessTests\|TaskFilesTests'` | passed after slice 4 (75 tests; two assertions moved to "Built-in Workflow") | build |
| the grep above | 314 lines at base, 309 after slice 1, 296 after slice 2, 249 after slice 3, none after slice 4 | build |
| `scripts/test_desktop.sh --skip-build --filter 'BacklogViewTests\|DashboardWorkspaceTests\|RoadmapViewTests\|CreateWaveSheetTests\|TaskRowTests\|TaskStandingTests\|WorkflowLineTests\|WorkNavigationTests\|WorkModelTests\|WaveDetailReadingTests'` | passed after slice 4 (162 tests) | build; CI runs the rest |
| Dev app in light, dark and an imported theme | looked at with the screen locked, on a private Machine: backlog, Portfolio Wave page, dashboard and Choose Theme in Loopflow Light, Loopflow Dark and Dracula (`scratch/qa.html`). Three fixes followed. Title bar, menus, hover, drag and terminal faces need an unlocked display | qa; review for the rest |
| `scripts/test_desktop.sh --filter 'TaskStandingTests\|TaskRowTests\|BacklogViewTests\|DashboardWorkspaceTests\|DesignDisciplineTests\|RoadmapViewTests'` | passed after the qa fixes (51 tests) | qa; CI runs the rest |
| `scripts/test_desktop.sh --filter 'TaskStandingTests\|NowProjectionTests\|TaskConceptTests\|WorkCLISubscriptionTests\|WorkPaletteTests\|DashboardWorkspaceTests\|TaskBriefTests\|RoadmapViewTests\|TaskRowTests'` | passed after slice 5 (64 tests; the first run failed `TaskConceptTests` on an abandoned Task reading "Waiting", fixed in `RoadmapTask.need`) | build; CI runs the rest, `WorkNavigationTests` and `DesktopHeadlessTests` among them, which mount the header |
| `scripts/test_desktop.sh --filter 'NowProjectionTests\|TaskStandingTests\|RoadmapViewTests'` | passed after realign deleted `nowGroup` for `RoadmapTask.need` (18 tests) | realign; CI runs the rest |
