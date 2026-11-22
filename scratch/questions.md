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

Made at qa with nobody present; `scratch/qa.html` has the captures.

27. A Dashboard tile's unstarted line reads "Not started" beside Start, not
    the Task's title: the tile's header already names the Task.
28. An empty repository shows the backlog's own empty ("Nothing is waiting in
    this repository", bold, an accent "New"), not the prototype's "No planned
    Work in this repository". Left as it is: the prototype's state in the
    backlog pane would sit beside "Conversation not open", two serif titles.
    Which should an empty repository show?
29. The request in New Task is semibold (`Typography.lede`, as `DESIGN.md`
    lists it); the prototype's textarea is regular. Left as `DESIGN.md` has it.
30. The two prototype sections stay until 28 and 29 are settled.

Made at the concept review of the built change with nobody present; the page
is `scratch/concept-review.html`, and its questions 1 to 6 are these.

31. Settles 28: an empty repository (no Waves, no Tasks, no conversation
    open) shows the prototype's one state in place of its page, until a
    conversation opens or a Task exists. Passed over: the state inside the
    conversation pane beside the backlog; the two panes as built.
32. An unstarted Task's pane is titled "Not started", not the Task's name as
    the prototype draws it: the brief beside it already sets the name in
    serif.
33. The backlog's empty takes the list form: one line in the secondary ink
    and a neutral button, for all four of its steps.
34. Settles 29: the typed request stays semibold, as `DESIGN.md` lists
    `lede`.
35. Settles 30: the two prototype sections are deleted in slice 5.
36. Settles 13: Rust sends no layer; "Product's default" names the scope
    whose new Tasks take the workflow up.
37. The Start added beside "No Graph has run." (21) goes: the bar above it
    has Start. "Open the conversation here." goes too.
38. A Start the Machine refused lands in creation recovery's notice, not
    "Not started." with Try again. Left outside this change; should a
    refusal keep the words and Try again, with only a lost answer going to
    recovery?

Made while building slice 5 with nobody present.

39. The plan's test for an empty repository ("no conversation attached,
    opening or failed") is built as written, and it means the released app
    rarely shows the state: a repository's conversation opens on its own
    there, so a fresh `git init` directory goes to its page. The Dev app
    opens nothing until a visit and does show it. Should an empty repository
    hold its conversation back until the button is pressed, in every build?
40. `WorkSurfaceView` lost its `NoPlannedWork` branch; `RoadmapView` keeps
    the title-only form, with or without a repository.
41. All four backlog empties end in a full stop, the search and claimed ones
    included, and `lf desktop capture -p backlog` prints the same line.
42. The prototype's CSS for the deleted sections stays in `desktop.css`,
    which `desktop.html` shares.

Made at compress after slice 5 with nobody present.

43. "Not filed." and "Not started." in New Task were hand-built copies of
    `FailureLine` in a strong face, as the prototype's `.bad` draws them.
    Both are `FailureLine` now, and `FailureLine` is strong everywhere, so
    the ten failures beside content ("Latest read failed.", "Project
    unavailable." and the rest) went from regular to semibold. Not seen in
    the Dev app. Passed over: a regular `FailureLine` in the sheet, which
    would have undone the weight qa saw there.

Made at the second qa with nobody present; `scratch/qa.html` has the captures.

44. A Files pane that could not open drew the bare message centred. It is
    `EmptyState(title: "Files did not open", machine:)` with no button: the
    pane has no re-read of its own.
45. Seen, not fixed: a Task started from New Task opened beside "Files did not
    open" three times in one window; the pane's recorded location was the
    repository while the Task had moved to its checkout. Reopening the app
    clears it. This branch changed nothing in `SessionsWorkspace.files(for:)`.
    Its own Task?
46. Question 39, seen: the same build under a bundle ID not ending in `.dev`
    opens a fresh repository straight on its page with a conversation started.
    Only the Dev app arrives on "No planned Work in this repository".
47. About twenty failures beside content are still bare `Text(error)`
    (`scratch/qa.html`, item 3). None was reached on screen, so none was
    changed. Convert them in this change, or leave them to a sweep?
48. The "Not started." pane with no Task selected could not be captured as
    committed: its scroll view meets the title bar and `cacheDisplay` returns
    white. Two local builds showed the view is drawn. Needs one look at an
    unlocked display.

Made at the concept review of the built change, second pass, with nobody
present; the page is `scratch/concept-review.html`, and its questions 1 to 4
are these.

49. Settles 39 and 46: the empty repository's state shows while no
    conversation is open, in every build, and the released app keeps opening
    a repository's conversation on its own. Passed over: holding the
    conversation back until "Open the repository conversation" is pressed.
50. Settles 47: the dozen failures beside content that are still bare text
    become `FailureLine`s in this change (slice 6), each with a word for
    what failed. Form checks, a file's read-only reason, the Create Wave
    forms, settings and telemetry stay.
51. The sidebar's "No working Tasks in this Wave" and "… in this list" take
    a full stop, and "Show working tasks" reads "Show working Tasks".
52. A stopped Graph's reason stays one sentence in the body face: Rust sends
    it as one string. Setting the Machine's part in mono needs two fields on
    the wire; outside this change.

Made while building slice 6 with nobody present.

53. `ProjectReadinessView` shows "Project not prepared." only for a
    transport or activation error. "No Project selected yet.", "Project
    activation failed." and the other readiness sentences stay warning text
    with Retry under them: they have no Machine message to set in mono.
    Should an activation outcome with no error also read as a failure?
54. The Files pane's error reads "Latest read failed.", not the plan's "File
    not read.": the same string carries "Live file observation unavailable:
    …". The recovered resource reads "Current work not read." in place of
    "Current work did not read."
55. `Views/WaveDetailPane.swift:248`, the Wave page's unavailable Tasks, was
    not in the plan's list and became "Tasks did not load." with the rest.

Made at the third qa with nobody present; `scratch/qa.html` has the captures.

56. The Open Work window opened 480 by 1,989 pt after slice 6 (240 before
    it): a `FailureLine` was the window's whole content and set its smallest
    size. Fixed with a minimum width on `WorkspaceOpeningFeedback`. Not done:
    a floor inside `FailureLine` itself, which would cover "The latest read
    of this repository failed." and "Layout not saved." in the main window
    without each site minding it, and could squeeze the line in a narrow
    tile. Neither of those two was seen on screen. Which?
57. The workspace's "Layout not saved: …" (`SessionsView.swift`) was plain
    text beside the Dashboard tile's `FailureLine` for the same failure. It
    is the same line now.
58. Question 45, seen again on the first Start in a fresh window, so every
    Start from New Task opens beside "Files did not open". Filed as Task
    ea8659; nothing in this change touches it.
59. The backlog's "Brief not saved · …", "Brief not restored · …" and "Order
    not saved · …" share the one-string notice that says "Filed fc6ad0".
    Left: a `FailureLine` there needs the notice to carry a failure apart
    from its message. Convert them in this change?

Made at the concept review of the built change, third pass, with nobody
present; the page is `scratch/concept-review.html`.

60. Settles 59 and adds four: the backlog's three failed notices, the
    roadmap card's unavailable Tasks (a warning triangle where the Wave page
    says "Tasks did not load."), its per-Task unavailable line, the
    Dashboard's membership reason and the Changes pane's "Diff settings
    unavailable" become `FailureLine`s (slice 7). The sweep read every error
    text and warning icon in the target, so this list is the last.
61. Settles 56: `FailureLine` gets no minimum width of its own; a site where
    it is the whole content sets the width, as Open Work does.
62. Settles 53: readiness sentences with no Machine message stay text.
