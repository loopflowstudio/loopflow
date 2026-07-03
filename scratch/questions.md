# Open Questions / Blockers

## BLOCKER: Wave has no roadmap or metrics — no safe move exists (2026-07-02, re-confirmed 2026-07-03)

The looping Goal agent ran one iteration and found nothing to act on:

- **Roadmap handle:** empty — no Asana backlog to read or pick from.
- **Metrics:** none configured — no way to measure whether a move advances the goal.
- **Wave memory / in-flight:** empty — no prior context or work to continue.

The goal contract forbids inventing work. With no roadmap task to dispatch and no
metric to drive, any move would be local optimization dressed as progress. Halted
per "record the blocker and stop."

**To unblock, wire one of:**
1. A roadmap handle (Asana project/section) holding at least one scoped task.
2. Metrics for the goal, so the loop has a target to measure against and can
   propose moves that provably advance the whole.

Until then the loop has nothing to advance.
