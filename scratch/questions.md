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
  stay. Pruning them is unsafe while the repository could switch back to
  Git: the kept observed heads are what lets the seed's captures win
  causally instead of becoming lost lines (`scratch/concept-review.html`,
  counterexample 2). Pruning would need the switch-back to treat a missing
  journal as "local wins", which is a product choice.
- **Publication gate** (concept review, chosen for review): Git's own
  outbox (unrouted changes or an unconfirmed push) on one 5 s loop, which
  replaces the 1 s loop and the global `planning`-revision gate. A failed
  publication retries within 5 s instead of 1 s.
- **One 5 s tick per foreground Session**, not shared across processes.
  Excluded here.
- **Tick decision is invisible from outside.** Publication writes are
  conditional, so an unchanged tick that publishes and one that only acquires
  leave the same store. The outbox read is tested in storage; the loop's use
  of it is not. A fetch counter would make it observable; not added.
