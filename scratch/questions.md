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

13. The prototype's unstarted pane says "the Wave's default". Taken: one
    phrase for the sheet and the pane, the scope's name ("Product's
    default"). The wire sends `repository` for a Wave's `graph:` and the
    repository's alike, so "the Wave's default" would be a guess; telling
    them apart needs a new `ConfigSource` or a scope field from Rust. Is the
    scope's name enough, or should Rust send the layer?
14. `docs/surfaces.md` draws the unstarted Task's bar with its neutral Start
    beside the pane's accent Start, as the plan keeps both.

Made at realign after the sheet slice with nobody present.

15. `StartingTaskView` keeps its place at the pane's top-leading corner when
    a start fails, where the prototype's failed state is vertically centred:
    the request is already there before the failure and should not jump. It
    has the prototype's parts and is not an `EmptyState`.

Made while building the states with nobody present.

16. The workflow's name in "It will run feature, …" is set in the sentence's
    face, not mono as the prototype draws it: `EmptyState.sentence` is a
    `String`. Making it `Text` would allow the mono run.
17. The unstarted pane no longer shows the Task's description (it is in the
    brief beside it) or the refusal under the button; a refused Start is off
    with the reason as its tooltip, as `docs/surfaces.md` says.
18. A failed conversation or Session open shows the error as the machine's
    message in mono, with no sentence above it.

Made while building the remaining states with nobody present.

19. Two small pieces joined `EmptyState.swift` though the brief says "add
    none": `.emptyLine()` (a list's one-sentence empty) and `FailureLine`
    (what failed, the message in mono, Retry) for a failure beside content
    that stays. Each replaced the same three modifiers at 7 to 15 sites.
20. New Task joined only "No planned or started Tasks." (`WaveTiles` has the
    model and the sidebar's Wave). "No Tasks in this Project.", "No current
    Tasks." and "No Projects in this Wave." sit in views with no `WorkModel`
    and a Wave that may not be the sidebar's, so they got wording only.
21. "No Graph has run." offers Start only for a Task that awaits its first
    start, under `runRefusal`; a started Task's empty log has no button.
22. "This repository did not open" in `SessionsView` gained Retry: the
    Machine and repository read that ran once on appear is now `readMachine()`.
23. "Workflows unavailable · …" sits inside a `Menu`, which draws plain text
    only; it reads "Workflows unavailable: …" with the message in the menu's
    face.
24. No "We do these now" pointer in `DESIGN.md` named code this change
    fixed, so none was removed.
25. "Task is unavailable" and "Session location unavailable" fill a pane, so
    they became `EmptyState` titles, not one-line sentences.

Made at realign after the remaining states with nobody present.

26. A retained Task whose planning is unavailable said so in a tinted banner
    with a warning icon ("Retained Task · planning unavailable"). It reads
    "Planning is unavailable for this Task." as a `FailureLine` with the
    reason in mono, and has no Retry: the page has no re-read of its own.
