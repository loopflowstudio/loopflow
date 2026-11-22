# Open choices, made for review

Each was decided so the work could continue; `scratch/plan.md` has the detail.

1. **`WorkflowSteps` stays, so one Desktop pointer survives.** The brief's
   "Done when" says DESIGN.md's Desktop pointers are gone. Folding New Task's
   `WorkflowSteps` into `WorkflowLine` needs a Graph definition's top-level
   steps on the wire (Rust, DTO fixtures), and the brief lists no New Task
   work. Left for its own Task. Should this change carry the wire change
   instead?
2. **`WorkSectionHeading` and `WorkDisclosureHeading` stay.** The brief says
   they go. Since #2016 they are sentence case; only the two hand-made
   uppercase variants are removed. Delete both and inline a plain heading at
   their nine call sites instead?
3. **Choose Theme keeps the tile grid and the per-side pickers.** The
   prototype draws a list of names and one mode. Only the "Mode" caption and
   the sample change. Rebuild the chooser to the prototype's two columns?
4. **Empty and failed states are untouched.** DESIGN.md describes one shared
   component; none exists and the brief says to add none. 14
   `ContentUnavailableView` sites stay.
5. **A contract issue's next step is "Copy".** The wire carries a summary
   and no recovery.
6. **Spacing ties snap down** (6 → 4, 10 → 8, 14 → 12). Slice 1 tests this
   on the backlog before the sweep uses it.
7. **`frame` widths and heights stay numbers**, and the discipline test does
   not check them.
8. **A blocked or waiting Task's reason shows on its sidebar row too.**
   `TaskStanding` gives every row the same line, so the sidebar gains one for
   a Task whose condition is not clear. Keep the reason to the Portfolio and
   the Brief instead?
9. **A click on a Task in the Portfolio opens it; `WaveWorkInspector` is
   deleted.** The Wave page loses the provider's name and the checkout's path
   as text. Keep select-then-open there?
10. **The order of program report, condition and lifecycle is decided in
    Swift.** Rust writes each source; none of them merges a program's report
    with planning. Should the planning wire carry one standing instead?
11. **"Available" counts Tasks you claimed.** Mine 2, Available 43 overlap,
    and an unclaimed row says nothing because "the Available count already
    does". Left as is. Rename it, or make it mean unclaimed?
12. **A Wave keeps two pages** (its repository view and the Portfolio's).
    Named in the plan as outside this change.
13. **Start's label for several Tasks.** The prototype gives only "Start with
    feature". The docs say `Start 2 with feature` for several on one workflow
    and `Start 3: feature 2, code 1` when they differ. Different words?
14. **A busy Task's word.** The prototype's tiles and theme sample say
    "Working"; the app's rows say "Busy" today, and the docs keep "Busy".
    Rename the word to "Working"?
15. **The Task pane and the jump palette keep their own words.**
    `TaskWorkflowHeader` says "Completed" and "Canceled" where the row says
    "Done" and "Abandoned"; the palette says "Not started" where the row says
    "Unstarted". Slice 1 left both. Should they print `TaskStanding`'s word?
16. **The Wave page's rows no longer name the owner or who moves next.** The
    old row read "idle · Ana · next: user". The shared row shows neither,
    as in the sidebar. Put the owner back on Portfolio rows?
17. **`DesignDisciplineTests` fails until slice 4.** It lists every literal
    left in other files (301 lines after slice 2). The branch is not green on
    that suite between slices.
18. **The backlog's checkbox always shows.** It used to appear on hover or
    once something was selected; the prototype draws it on every row. Hide it
    again until hover?
19. **The plain-text backlog listing keeps " · " between columns.** Only the
    words inside a column changed ("Idle, at demo, PR #12", "claimed by
    you"). Use another separator there?
20. **Abandon… is a neutral button**, as the prototype draws it, not
    `.destructive`; a dialog still confirms.
21. **`Notice.swift` is shared with iPhone.** `DesignDisciplineTests` reads
    `swift/Loopflow`, so its four paddings (14, 10, 8, 18) snap in slice 4
    and the iPhone notice moves by 2 pt with the Mac's. Exempt the file
    instead, since iPhone is outside this change?
22. **Count pills stay.** `WaveRow.swift:112` and `RoadmapView.swift:370`
    draw a count in a filled capsule, and `TaskChangesView.swift:240`,
    `TelemetryDashboardView.swift:216` and `NewTaskSheet.swift:85` use one
    too. The backlog's accent capsule went; no rule or test names the rest.
    Make them plain counts in the sweep?
23. **An unsaved dashboard has no serif name.** The prototype's note says
    "You named it, so it gets the 26 pt step"; a query opened and not saved
    is headed by its query alone. Give it a word such as "Unsaved Dashboard"?
24. **New Wave's button says "Create"**, as `swift/README.md` does; it said
    "Create wave". Its title is "New Wave" in serif at the title step, in the
    sheet and in the sidebar's popover alike.
25. **The Now list's count keeps its pill**, snapped from 1 pt to 2 pt of
    vertical padding (question 22 covers whether the pills go).
26. **Refresh on the roadmap is the one button** holding an icon, where it
    was a borderless icon.
27. **Unstyled buttons outside menus are the one button now.** The brief
    named 22 system button styles; a `Button` with no style is also the
    system's. Settings, the sheets, the Portfolio controls, the Graph and
    Workflow inspectors and inline Retry, Refresh and Done got
    `LoopflowButtonStyle`; Save, Apply, Rename, Retry-after-failure and
    Comment are each surface's accent, Remove is `.destructive`, and Cancel
    on the Rename and Remove sheets is neutral where it was plain text.
28. **Middle dots in view strings stay.** About 60 remain
    (`WorkPalette` details, `TelemetryDashboardView`, `ReviewStreamView`,
    `GraphDiagramView`, `ConfigurationSettingsView`, `TaskFilesView`);
    several have tests and CLI twins. Only the two the plan named changed
    (the roadmap's unavailable Task line, the metric detail). Sweep the rest
    in their own Task?
29. **"Built-in Graph" and "Built-in Workflow"**, in sans, where the
    inspector said "builtin Graph" in mono; a definition's path stays mono.
30. **Keyboard shortcuts are sans.** "⌘K" in the shortcuts list and "⌘N" in
    the sidebar are not something to paste into a terminal.
31. **The activity label reads "Wave, Product"**, and the pane's scope title
    still reads "Wave · Product" (question 28).
32. **The scrolling Task detail's bottom margin is 32 pt**, where it was 72.
33. **The browser pane's Back, Forward and Reload are bordered icon
    buttons**, like the roadmap's Refresh (question 26).
34. **Four AppKit views keep a system font set by number.** The file editor
    (`TaskFileDocument.swift:133`), the history filter's counts
    (`TaskHistoryFilter.swift:128`), the terminal search count
    (`TerminalSearchBar.swift:46`) and the licenses text
    (`OpenSourceLicenses.swift:72`) use `ofSize:`, which
    `DesignDisciplineTests` does not read, so DESIGN.md's "fails on a font
    outside the ramp" holds for SwiftUI only. Found after slice 4; the plan
    never listed them. Move them to the bundled faces and add `ofSize:` to
    the test, here or in their own Task?
35. **The Files tree's changed-directory badge is a typed "•"**
    (`CheckoutTree.swift:159`), in the mono column that holds M, R and U. The
    plan named the unsaved-draft "•" only. Leave it as that column's mark, or
    make it a symbol?
36. **`WaveLens.forTasks` has no caller.** Compress deleted `forTask`, whose
    last caller was the Wave page's old row. `forTasks` and `fold` were
    already unused at the base, with six tests holding them. Left, as outside
    this change. Delete them and their tests?
37. **An unstarted Task whose next move is Start reads "Unstarted", with no
    line.** The server marks every such Task `waiting` with the reason
    "ready", so a whole backlog read "Waiting / ready" in amber on the Wave
    page and the Now list. `TaskStanding` now treats `waiting` as a need only
    when the Task has started or its recommended action is not Start. The Now
    list still files those Tasks under its "Waiting" heading, which comes
    from the wire. Change the heading, or the server's condition?
38. **A started Task parked at an interactive step reads "Waiting" over the
    line "ready".** The line is `next_move.reason` verbatim. On a dashboard
    tile it also replaces the workflow sentence. "ready" says nothing a
    person can act on; "Waiting for you at demo" is what the workflow bar
    says for the same Task. Left: the wording is the server's (question 8).
39. **A dashboard tile shows its workflow sentence only for a Task opened in
    this window.** `taskWork` fills from the selected Task's subscription, so
    a tile for a Task never opened shows its row alone. Left as outside this
    change; is the sentence on every tile worth a read per tile?
40. **The Now list omits Tasks that wait only to be started** (slice 5).
    It lists a Task when its row says it needs someone. Answers question 37's
    heading half in Swift; the server's condition is named in the plan as
    outside this change. Keep them listed under another heading instead?
41. **A finished Task's page says "Done" or "Abandoned"**, where it said
    "Completed" or "Canceled", and the jump palette says "Unstarted", where
    it said "Not started" (slice 5; decides question 15). Keep the older
    words on the page and move the rows to them instead?
42. **A parked Task's row still reads "Waiting" over "ready"** after slice 5;
    only the tile regains its workflow sentence. The line is the server's
    reason (question 38). Drop the line when it is the bare status word, or
    wait for the wire to carry the sentence?
43. **A finished Task is never "Waiting".** The server marks an abandoned
    Task `waiting` with the reason "ready", so its row and its page read
    "Waiting" over "ready". `RoadmapTask.need` now returns nil for `waiting`
    on a done or abandoned Task; `blocked` and `unknown` still show. Found
    in slice 5 when the page took the row's word. Should a finished Task
    show a blocked condition either?
44. **A Task's page reads its programs.** `TaskWorkflowHeader` prints the
    row's word, so an open Task whose program reports says "Busy" or
    "Available" where the page said "Active" or "Idle", and "Done" and
    "Abandoned" are in secondary ink on the tinted header, where the word
    was green or red. Keep the lifecycle word and its color on the page
    instead?
45. **The Brief's lead keeps the lifecycle word.** A Task parked at demo
    reads "Waiting" on its row and its page and "Active, at demo. ready." in
    its Brief (second qa run). The lead is #1949's sentence; slice 5 moved
    only the line under it. Should the lead say the row's word?
46. **A placed Task that never ran reads "Waiting / ready"** and is on the
    Now list. The server calls it `idle`; `RoadmapTask.need` exempts only
    `unstarted`. Opening an unstarted Task by its link gets there. Left to
    the Rust Task under "Outside this change"; widen `need` to `idle` Tasks
    whose Graph has not run instead?
47. **A Portfolio row offers "Start" for a parked Task** where its page
    offers "Run" (`RoadmapTaskAction.label`). At the base too. One word?
48. **The pane's context line stays mono.** "729776 Weekly digest email ·
    atlas" in a pane header is mono prose; in sans it truncated to one letter
    and a whole-or-nothing version widened the pane (second qa run, both
    reverted). It needs someone who can watch the strip lay out.
49. **Awaiting Start means no Graph has run** (slice 6; decides question
    46). `need` reads `latestGraphProcess == nil` where it read the
    `unstarted` lifecycle, so a Task someone opened and never started reads
    "Idle" and is off the Now list. Should a claimed Task with a checkout
    and no run count as waiting on its owner instead?
50. **The Brief's lead says the row's word** (slice 6; decides question 45):
    "Waiting, at demo. ready." where #1949 wrote "Active, at demo. ready."
    It ignores a program's report, so an open Task whose program is Busy
    reads "Busy" in its header and the Task's own word in its Brief. Should
    the lead follow the program too?
51. **A Portfolio row says "Run" for a Task whose Graph has run** and
    "Start" before (slice 6; decides question 47), as the Task's page does.
52. **"ready" is no longer printed for a Task that needs nobody** on the
    Task detail's line and the activity pane (slice 6). A parked Task still
    reads "Waiting" over "ready" (question 42).
53. **The backlog still calls an opened, never-run Task started** (slice 6).
    Its rows, counts and action bar split on `awaitsStart` (no checkout),
    not on `graphHasRun`, so such a Task shows its word there and is not
    offered in the bar's Start. Left as it is; move the backlog to
    `graphHasRun` too?
54. Sync with main uses the shared Task row on Mac and iPhone. The Mac supplies its planning condition. Notice uses main’s padding steps.
55. **The Brief gives no reason for a Task that needs nobody** when its next
    move only repeats the condition's reason (third qa run): "Idle." where it
    read "Idle. ready." A moving Task keeps its next move's reason. A blocked
    Task's Portfolio row still offers only "Worktree" while its page offers
    "Run"; at the base too. Should the row offer Run?
