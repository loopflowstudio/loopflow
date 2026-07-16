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

## Open for the parent Project Session

**Decision 6 — the dead-parent case.** Recovery runs on the Project runner's 5s
tick, which removes the flow-turn dependency but not the live-parent dependency. A
Task whose Project Session is itself dead still strands. Fixing that means the
wave tier (the only self-reviving tier) and would be a second dispatcher, which
the directive forbids. Chose the bounded win; flagged rather than hidden.

If the dead-parent case is the real target, this design is one increment short and
should be re-scoped before implementation.
