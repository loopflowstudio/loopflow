# Open choices (923677)

Made during kickoff with nobody present; each is reversible and marked in
`plan.md`.

1. The brief says of the shared components "Use them; add none", and also
   "becomes the shared state component". No state component exists on main.
   Taken: `PaneEmptyState` is rewritten and renamed `EmptyState` in `Design/`,
   so the count of components does not grow.
2. The sheet's workflow popover (search field, a summary line per workflow)
   becomes an ordinary `Menu` of `StartFlowItems`. Search and summaries go.
3. File stays in the sheet though the prototype draws only Cancel and Start.
4. The catalog gains a required `workflow` field (steps and the "You take
   part at …" sentence, written in Rust). An app from this branch needs this
   branch's `lf`.
5. Compact dashboard slots lose their icon-only fallback; the action alone
   stays, with the title as its help.
6. The prototype's "A Task that has not started" state replaces
   `TaskStartOffer`, in the Task's pane and in dashboard tiles; the workflow
   bar keeps its own Start.
7. The two prototype sections are not deleted in this change's first pass;
   they go once the Dev app check has compared the screens against them.
8. "Open its conversations" and "Open another repository…" are the
   prototype's proposals. Each ships only if the view can already reach that
   action; otherwise the state has one button.

Made during concept review with nobody present; the page is
`scratch/concept-review.html`, and its numbered questions match these.

9. An unstarted Task's second button reads "Start with…", not the
   prototype's "Choose workflow": picking an item starts the Task.
10. A top-level step that contains a stop (inside an XOR path) is marked with
    the person icon, on started Tasks' lines too, so the line, the sentence
    and the menu's stop count come from one list.
11. An unstarted Task's bar draws the step line from the catalog; "Takes up
    feature when it starts" goes.
12. The unstarted state's wording is two sentences, the second Rust's
    ("You take part at …"), where the prototype joins them with "and".

Made while writing the docs with nobody present.

13. With a Wave chosen, the sheet today says "Product's default" (the scope's
    name), and the plan's unstarted pane says "the Wave's default". The docs
    show both as written. Should one phrase serve both, and which?
14. `docs/surfaces.md` draws the unstarted Task's bar with its neutral Start
    beside the pane's accent Start, as the plan keeps both.

Made at realign after the sheet slice with nobody present.

15. `StartingTaskView` keeps its place at the pane's top-leading corner when
    a start fails, where the prototype's failed state is vertically centred:
    the request is already there before the failure and should not jump. It
    has the prototype's parts and is not an `EmptyState`.
