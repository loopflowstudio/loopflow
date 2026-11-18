# LOO-938 open questions

- **Git onto `tracker_actions` was passed over** (plan, "Decision"). Is a
  follow-up still wanted for Git "open" lines in `lf repo plan`, or is the
  shared `TrackerActionStatus` view enough?
- **Stored tracker.** `repository_plans.planning_tracker` is a copy of one
  config fact so the writer can skip the journal. Alternative was capturing
  lazily at exchange time; rejected because capture time would become the
  causal clock (an offline edit would beat a later one made elsewhere) and a
  full-repository capture per publication breaks the 10k-Task load bound.
- **Linear-only fields in the Git document** (`task_linear_id`,
  `project_linear_id`, `task_url`, `task_team`, `project_initiatives`,
  `project_teams`): journaled and published to peers today, although the doc
  says the Git document "contains no Linear observations". Dropping them
  changes what released peers read; left for the store review Task.
- **Switched repositories' journal rows** (manabot 13k, kata, etude-studio)
  stay: a migration cannot read their config. They could be pruned once
  their recorded tracker is `linear`, in a later Task.
- **Acquisition every 5 s per foreground Session**, not shared across
  processes. Excluded here.
