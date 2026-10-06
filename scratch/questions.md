# Open questions and assumptions (LOO-382)

Recorded 2026-10-05 during kickoff. Nobody has confirmed these.

1. **Store revisions are a shared-store change.** The plan adds a
   `store_revisions` table and triggers. Infrastructure owns execution and
   store semantics; the Task asks for coordination. Assumed acceptable as one
   draft migration on this branch. Unresolved: whether Infrastructure wants to
   own the interface or its domain list.
2. **Roadmap read cost has no owning Task.** Wave memory says so (LOO-375 owns
   `wt list` only). This plan takes the completion-gate Exec scan because the
   latency acceptance cannot be met without it. Other read costs stay unowned.
3. **Resolved (Jack Heart, 2026-10-05).** Desktop reacts to the store only;
   Linear arrives through sync. Unowned: nothing schedules that sync for
   Desktop's benefit today. Jack named a schedule, Desktop-triggered syncs and
   webhooks as options, preferring webhooks. No Task filed.
4. **Resolved (Jack Heart, 2026-10-05).** The budgets stand as targets pursued
   for about an hour; a miss ships with measured numbers and a follow-up Task.
5. **Folding `monitor active --watch` into the workspace stream** reshapes a
   proven reader. Chosen to end with one process per window; if it destabilizes
   active-Session discovery, the fallback is to leave that reader separate and
   say so, not to keep both paths for the same part. As built the fallback
   was taken: `active` is still its own process, without evidence that folding
   it in destabilizes anything.
6. **Per-read Exec recording.** ~63,000 of 65,043 daily Exec rows come from
   Desktop polls. This plan removes the polls; it does not change whether
   one-shot read commands record an Exec. That stays a question for
   Infrastructure.
7. **Desktop's Create button uses `task create --run`.** The row will appear
   from the commit regardless of the run. Whether Desktop should also offer
   create-without-run is a product choice outside this plan.

Added 2026-10-05 in design review. Still unconfirmed.

8. **Resolved (Jack Heart, 2026-10-05).** One landing.
9. **Should a one-shot read bump `execs` at all?** Clicking a file or loading
   comments in Desktop runs `lf`, writes an Exec and, as designed, re-runs the
   planning projection. Harmless at 2,067 a day; it is also the reason
   question 8 has a cost. Narrowing the `execs` domain to Execs the planning
   conditions can read (those inside a Task checkout, or unfinished) would
   remove it. Not designed here.
10. **Narrowed in implementation, 2026-10-05.** The first predicate exempted
    every `:events.jsonl:` observation, including provider attempts that Work
    activity and Wave history read. It now exempts only the types the summary
    readers skip, plus usage. `session list` fields are tested; Work activity
    and Wave history are not.

Added 2026-10-05 in implementation. Nobody has confirmed these.

11. **Three indexes joined the shared store** beside `store_revisions`:
    `execs_unfinished`, `session_events_exec`, `flow_events_exec`. They serve
    the completion gate. Infrastructure has not seen them (question 1).
12. **The watch reuses Git answers**: repository layout for ten minutes,
    checkout contents for one. Only the watch process does; one-shot commands
    ask Git every time. This is what makes a planning reading 2 s instead of
    7.5 s, and it is why Git-only changes show later than they did.
13. **Planning is re-read every five minutes without a commit**, down from the
    design's sketch of a slow clock. Shorter costs a 2 s reading each time.
14. **Process activity is read every 2 s inside the watch**, as the deleted
    loop did by spawning `lf`. It observes every provider process on the
    machine, so its frames keep arriving while agents work. Idle CPU of the
    watch is unmeasured.
15. **The follow-up Task for the latency miss is not filed.** Jack asked for
    one when the budget is missed; filing belongs with whoever lands this.
16. **Session fixtures read once.** UI-test modes without a reader used to
    re-read Sessions every 2 s; they now read at launch and after local
    actions. No UI test was run.

Added 2026-10-05 in realign. Nobody has confirmed these.

17. **The Task-files comparison still re-reads every 10 s through `lf`** while
    it is shown (LOO-327). Each read writes an Exec and re-runs planning in
    the watch. Whether this Task removes that loop, or question 9 narrows the
    `execs` domain instead, is undecided.
18. **Token totals in a Wave's Session history lag** until the next displayed
    change or five minutes, because usage moves no revision. A `usage` domain
    that invalidates only the Wave part would fix it at the cost of re-reading
    that part every few seconds while agents work. Not built.
19. **One loop reads the parts in turn.** A Session commit that lands while
    planning is being read waits about 2 s for it on Jack's store. Reading
    Sessions on its own connection would remove the wait. Not built; it is
    the same gap as the planning reading being 2 s.
20. **The write-to-visible benchmark writes rows with `sqlite3`**, cloning a
    current Task's planning item, because `lf task create` needs Linear. It
    measures the reader and the window, not `lf`'s own write path.
21. **A frame from another Home is applied after dropping the old Home's
    content**, where the plan said it would be ignored. The reader exits on a
    Home change, so one reader never sends two Homes; the drop is what a
    reopened reader needs.
22. **A Task created without a Run never reaches the left pane.** The outline
    lists a Task only when work on it has started or it has an open Session
    (`WorkspaceTask.inWorkingSet`; Jack Heart accepted "started Tasks only"
    on 2026-09-25). LOO-382 asks for a new Task to appear in the left pane
    "without an execution-start prerequisite", and its demo creates one with
    no `--run`. Both cannot hold. The stream delivers the Task: it shows in
    its Wave's Task list at once. Whether the outline should also list
    unstarted Tasks, or only new ones, is Jack's to decide; nothing was
    changed. Desktop's own Create uses `--run`, so that Task appears once
    its Run is recorded.

Added 2026-10-05 after Jack Heart read the PR walkthrough.

23. **Direction (Jack Heart, 2026-10-05): seeing which Waves and Tasks
    are current is a basic part of the UX, and the architecture should make
    it very simple and fast.** Two principles: move each thing to its right
    owner, Swift or the database; build on the performant, scalable
    architecture from the start. The database serves the Loopflow UX overall.
    Measured since: the list is slow because of one per-Task query and
    cold Git, not because it carries detail (plan, "Simple, fast planning
    read"). **Chosen without Jack's confirmation:** keep the wire and Swift
    types, read in bulk, store no derived state and add no change log. Jack
    said "i think we want it in the db" about derived state; that is not
    built. He also said not to over-index on a keyed Swift store.
24. **Built: a failing part backs off**, 1 s doubling to 60 s. The cap is a
    guess. A commit or a request still reads the part at once, so a part
    that fails while agents commit is read as often as a healthy one.
    Jack added: "this db's only purpose really is to serve this UX." Read as:
    display-shaped rows are the store's primary tables, not a cache beside
    them. The store also holds execution authority (Flow position, claims,
    landing locks); how those relate to display rows is not worked out.

Added 2026-10-05 building the planning-read slices. Nobody has confirmed these.

25. **The planning reading still runs statements per unfinished Task**, so
    the plan's rule is met for Exec membership only. Stopped there because
    the projection is 190–200 ms against 300 ms, and the remaining reads are
    the completion rule `lf task complete` also uses. Reading them in bulk
    is the next step when the reading stops fitting; whether to do it now,
    or to store derived rows as Jack described (question 23), is his call.
26. **A first reading is 4.2–4.6 s and was left there.** About 1.7 s is Git
    asked about 32 existing checkouts in turn; asking at once was not built.
    0.6 s is a foreign-key check a development build runs on every store
    open, twice per start; whether an installed build pays it was not
    checked.
27. **`unresolved_execution` is a required field on the Task condition.**
    Workspace text saved by an earlier build lacks it, so the first launch
    after upgrading should show no saved planning until the reader answers.
    Expected from LOO-376's rule that saved text goes through the live
    decoder; not exercised.
28. **The outline's "has open Sessions" and "started" tests stay in Swift.**
    Slice 5 moved only the finished-Task rule. Moving the rest means Rust
    joining Sessions into the planning part.
