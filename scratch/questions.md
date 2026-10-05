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
