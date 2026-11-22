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
