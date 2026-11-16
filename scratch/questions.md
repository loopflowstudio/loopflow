# LOO-935 open choices (headless kickoff, 2026-11-16)

- **Pane layouts move to Desktop files; `lf open` stops mirroring them.** The
  other option keeps the `work_pane_layouts` row and drops its triggers. That
  stops the planning wake but keeps one layout per repository for every
  client, so the outcome "no longer assumes one client per repository" isn't
  met. The file route was taken. The cost: Rex/Herdr/cmux `lf open` uses
  `task_layout` or the builtin instead of what Desktop last showed.
- **Dashboards are screen state but stay on the Machine for now.** The default
  taken is to drop their planning triggers. Still open: should saved
  Dashboards be per person, per client or per Machine, and are they used at
  all? Both tables are empty on this store.
- **Nothing is dropped in this Task.** Every dead member stopped being read
  on or after 2026-10-09, inside the 30-day window of builds that may still
  run. The bytes shrink when the marked contracts run, from about 2026-12-17.
- **Who schedules `due_removals.py --start`.** The choice is a weekly
  LaunchAgent on the Machine that runs Tasks, installed at verify. lf gains no
  scheduler of its own (`docs/dev/delivery.md`: scheduling is the person's own
  line).
- **Account marks depend on #1953 (LOO-913) landing first.** If it lands
  after this Task, its own comment-only migration has to carry them.
