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
- Removing or renaming a Wave no longer touches Linear (the Initiative
  archive/rename went). Its linked Linear Project is left as it is; archiving
  it on removal would be a new tracker action.
- Relinking a Wave whose Project Loopflow already made leaves that made
  Linear Project in Linear, empty.
- `WorkValue::ProjectInitiatives` stays deserializable because peers on
  released lf still send it in Git and hub exchange; nothing here produces it.
- This repository's own `.lf/wave/*/config.yaml` files still carry
  `linear: initiative:`; released lf reads them, the branch ignores them.
- A Wave removal summary now always says "N Project(s) stay readable in
  history", since every Wave has its Project; `WaveRemoval.projects` equals
  the number of retiring Waves. Kept as is; dropping the line and field is a
  wire change for another pass.
- QA 2026-11-23: renaming a Wave leaves its local Project's name as it was
  (`lf wave rename scratchpad sandbox` → `Project scratchpad (local)`). No
  Project rename path exists; following the rename (and a tracker `update`
  for a made Linear Project) is a product change, not made here.
- QA 2026-11-23: Desktop's Wave pane still has a **Projects** button listing
  the Wave's Projects; with one per Wave it lists one. Left as is.
- QA 2026-11-23: a `create` queued for a Wave's Project before `lf wave
  remove` likely still sends (read from the code, not run: removal does not
  retire queued actions); only Projects of Waves removed before the tracker
  was connected are skipped now.
