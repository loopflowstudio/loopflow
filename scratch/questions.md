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
   say so, not to keep both paths for the same part.
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
10. **The transcript predicate is a sketch.** See finding 7 and Risks in the
    design. Nobody has checked every `observed` receipt key that `session list`
    reads.

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
