# Open questions — W2-267

## Resolved during kickoff (recorded because both inputs were partly wrong)

**Did the 13 stranded Sessions have open PRs?** Yes — but that is not why they
stranded. W2-135 reached `failed` via `ops/task.rs:1806`, which is unreachable
when the PR phase is Open (`:1765` routes Open → `Waiting`). It crashed while
**Publishing**; `reconcile_task_pr` observed the GitHub receipt later and moved
the phase to Open. PR phase is a lagging, mutable signal that changed *after* the
strand formed. Assumption taken: gate recovery on durable status, not PR phase.
This satisfies the parent's "regardless of settlement" while preserving W2-129.

**Should `status_reason` be parsed to classify terminal failures?** No. It is a
free-form `String` (`task/mod.rs:631`) written from ~6 sites. `outcome.kind`
already carries the distinction structurally: `Lost` = Loopflow's reap verdict
(recoverable), `Failed` = the body's own recorded verdict (not blind-retryable).
Assumption taken: the directive's "read `status_reason` before retry" is honoured
by reading the outcome tag, which is the same information without the string
matching.

## Resolved at review (2026-07-16)

**Decision 6 — the dead-parent case.** Reviewer: keep the bounded win, do not
re-scope. Filed as `7e7be305-83fb-4689-ac7e-c1260d962924` ("Recover a Task Session
whose parent Project Session is dead"), carrying the reviewer's caveat that the
observed harm — Sessions frozen for *hours* — fits the dead-parent case better
than the flow-turn case, so this increment may be the smaller half.

**Predicate vs metric.** The first draft keyed recovery on `is_process_active()`
(`Starting | Running`), which excludes `Failed` — and therefore excluded W2-135,
the very row the Measure query counts. Resolved by including `Failed` +
`outcome::Lost`; safe because `status::Failed` and `outcome::Failed` are different
axes and the tag already discriminates W2-135 (recoverable) from W2-212 (terminal).

## No open questions

Both required review changes are applied. Nothing blocks implementation.
