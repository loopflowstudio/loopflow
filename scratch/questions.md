# LOO-584 open questions and assumptions

- Assumed (kickoff 2026-10-25): a non-person removal on a green head becomes a ci-fix incident (`merge-request (<cause>)`), and ci-fix runs `lf arm`. Release no longer re-arms directly. This costs one agent run for a harmless drop but keeps every loop bounded by incident dedupe. A direct re-arm is cheaper but has no bound.
- Assumed: Loopflow disarms outside a CI repair (lf sync/commit on an armed Task, mode → User) are the person's decision and stop the landing, as they do today.
- Open for Jack: whether a dequeue in the seconds between a repair's `lf arm` and its Process exit reading as ByLoopflow is acceptable, or whether it warrants recording Loopflow's own disarms durably.
