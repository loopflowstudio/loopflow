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
- **`lf desktop layout` keeps Dashboard slots only.** The plan removes
  `desktop.layout`/`.layout.save`, but `query:<id>` slot layouts go through
  the same command and Dashboards stay. The docs keep `lf desktop layout
  query:<id> [--save]` and drop `task:`/`wave:`/`repository` selectors. If
  implement removes the command outright, slots need another verb
  (`lf desktop dashboard slots` already exists; check whether it duplicates).
- **Schedule shape for `due_removals.py`.** Docs give `install`/`uninstall`
  subcommands (mirroring `disk-cleanup.sh`), Monday 09:00, log at
  `~/Library/Logs/loopflow-due-removals.log`. All three are picks, not asks.

## Steers (2026-11-16, word for word)

- Jack Heart: "You can drop everything old in this task as a special case".
- Jack Heart: "we can drop the thing where lf open does anything other than
  just open the task session in other surfaces."
- Jack Heart: "we dont need to try to copy our desktop layouts".
