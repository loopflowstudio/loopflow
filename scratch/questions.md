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
