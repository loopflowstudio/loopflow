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
- Open (QA 2026-11-20): a Linear-connected repository has no Git or hub plan
  exchange (`plan_git::exchange_all` skips `tracker::connected`), and the
  Linear import reads issues per Project only (`ListProjectIssues`). So on
  another Machine a Task in an ancestor's Project lists under that Project's
  Wave, and a Task with no Project never shows up. On main every Wave Task had
  a Project, so connected Wave Tasks crossed Machines. LOO-980 rules out a
  marker ("No Linear attachment") and removes team-issue ingest; its "own
  Machines exchanging through Git on a tracker repository" is where
  `tasks.wave_id` would cross. Its "Wave membership is derived from the link,
  not stored" contradicts this branch's `tasks.wave_id`. `docs/work.md` now says this.
