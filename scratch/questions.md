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
- Migration-made Projects are ordinary local Projects. In a connected
  repository the drain gives each a Linear `create` on the next export (the
  same as a Wave made now); their UUID is the Wave's, so two Machines' creates
  name one Linear Project. Retired Waves get none.
- An arriving Project record (Git or hub exchange) for a Wave that already
  holds a different Project fails the unique index. Only a released build can
  send one (it makes Projects per chapter); not handled here.
