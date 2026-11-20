# LOO-995 questions

- The plan says "A Task with no Project sends nothing to a tracker." Built more
  narrowly: only a Task *in a Wave* with no Project stays out. Standalone Tasks
  already go to Linear as team issues, and stopping that changes behavior
  this slice doesn't own. Widen it if standalone Tasks should stay local too.
- A retried creation compares the stored intent without its Wave: a retry
  naming another Wave reuses the first Task.
- Already true before this change: every Task listing (`tasks_where`) reads
  `TaskIdentifiers::read`, a full `SCAN tasks` (`SELECT lower(substr(id,6)),
  issue_identifier FROM tasks`). The Wave filter itself uses `tasks_wave`.
