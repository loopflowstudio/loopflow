# LOO-995 questions

- Concept review chose: every Task in a connected repository goes to the
  tracker, Project or not (plan, "Made for review"). Standalone Tasks unchanged.
- A retried creation compares the stored intent without its Wave: a retry
  naming another Wave reuses the first Task.
- Already true before this change: every Task listing (`tasks_where`) reads
  `TaskIdentifiers::read`, a full `SCAN tasks` (`SELECT lower(substr(id,6)),
  issue_identifier FROM tasks`). The Wave filter itself uses `tasks_wave`.
- `lf task status --json` gives `task.wave` as the leaf name (`refunds`),
  `lf task show` gives `execution.wave` as the path (`billing/refunds`).
  The installed build does the same; not changed here.
