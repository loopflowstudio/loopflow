# LOO-584 open questions and assumptions

- Assumed (kickoff 2026-10-25): a non-person removal on a green head becomes a ci-fix incident (`merge-request (<cause>)`), and ci-fix runs `lf arm`. Release no longer re-arms directly. This costs one agent run for a harmless drop but keeps every loop bounded by incident dedupe. A direct re-arm is cheaper but has no bound.
- Assumed: Loopflow disarms outside a CI repair (lf sync/commit on an armed Task, mode → User) are the person's decision and stop the landing, as they do today.
- Open for Jack: whether a dequeue in the seconds between a repair's `lf arm` and its Process exit reading as ByLoopflow is acceptable, or whether it warrants recording Loopflow's own disarms durably.
- Assumed (implement 2026-10-25): resuming an open release PR (explicit `lf release run` or a scheduled firing) arms it when its removal reads Manual or Unknown. This keeps ManualMergeRemoval's "Run `lf release run` to resume" true and matches the old `armed_at = now` reset on resume. A dequeue during the run still stops it. ByLoopflow, Evicted and Merged are left to the shared decision.
- Assumed: Unknown's synthetic check is `merge-request (unexplained)`; ByLoopflow's is `merge-request (repair-disarmed)`.
