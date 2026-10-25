# LOO-573 assumptions (kickoff 2026-10-24)

- Item 4: a manual dequeue ends `lf release run` with an error naming the dequeue,
  rather than deferring to the next cron firing. A scheduled run the next day arms
  again as a new request. Simpler and honors the person; revisit if Jack wants a
  dequeue to suppress later scheduled runs too.
- Item 3: `lf release notes` promotes the invoking checkout's own
  `RELEASE_NOTES.md` and `release/unreleased/`, while the change range comes from
  `origin/<default>`.
- Item 1: the `--flow` alias and serde aliases stay until a removal Task confirms
  every loaded launchd job has been reloaded with `--graph`.
- Item 2: receipt-keyed accounting writers (`settle`, `select`, `record_target`,
  `record_telemetry`, `authorize_replacement`, `overlap_continuation`) now take the
  repository; `lf wave cron disposition` resolves it through the Wave's cron authority.
- Item 4: a person's removal is `RemovedFromMergeQueueEvent.reason=manual` or
  `AutoMergeDisabledEvent.reasonCode=manually_disabled` after this run's latest arm.
  Loopflow's own disable-and-rearm happens before the arm timestamp, so it never
  counts. A different `lf` process disarming the PR on the same account would.
