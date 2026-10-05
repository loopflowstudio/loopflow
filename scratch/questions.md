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
3. **Linear-side changes are excluded.** "Other connected clients" is read as
   other processes and windows writing this Home's store. A Task created in the
   Linear web app appears only after a sync commits it. If Jack means Linear
   too, a provider observation path is additional scope.
4. **Budgets are proposed, not published.** ≤ 1 s create-to-visible and
   ≤ 500 ms Session updates are this plan's numbers. Wave memory requires
   published budgets before scoring.
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

8. **One landing or three.** The plan lands slices 1–4 together, reasoning that
   anything less leaves a polling fallback. A per-part cutover does not: each
   landing replaces one loop with its stream part and deletes that loop, so no
   part ever has both. Possible landings: (a) store revisions, the
   completion-gate fix, the `planning` part and Desktop's planning cutover —
   the create-to-sidebar demo; (b) `sessions` and `task`; (c) `activity`,
   `active` and the benchmark. Cost of splitting: between (a) and (b) the 2 s
   Session poll still writes ~25,800 Execs a day, each bumping `execs` and so
   re-running the planning projection roughly every 2 s at ≤ 300 ms — about
   15% of a core, idle. Cost of not splitting: one change spanning a migration,
   a new command, a hot-path read and the window's refresh owner, reviewed and
   reverted as a unit. Jack's call.
9. **Should a one-shot read bump `execs` at all?** Clicking a file or loading
   comments in Desktop runs `lf`, writes an Exec and, as designed, re-runs the
   planning projection. Harmless at 2,067 a day; it is also the reason
   question 8 has a cost. Narrowing the `execs` domain to Execs the planning
   conditions can read (those inside a Task checkout, or unfinished) would
   remove it. Not designed here.
10. **The transcript predicate is a sketch.** See finding 7 and Risks in the
    design. Nobody has checked every `observed` receipt key that `session list`
    reads.
