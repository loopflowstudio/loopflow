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
