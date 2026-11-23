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
- QA 2026-11-23: renaming a Wave left its local Project's name as it was.
  Slice 4: the Project's name follows the rename, linked or not; a linked
  one's new name is sent to the tracker.
- Renaming a Wave overwrites a local Project name someone edited by hand
  (none of the commands here sets one apart from its Wave's).
- Concept review after slice 3: a queued Task `create` snapshots
  `task_project`; `retire_wave_in` rewrites it on removal. Reading fields at
  send time instead is a tracker-drain redesign, its own Task (page question 5).
- Slice 4 kept the Backlog Wave menu's **Projects** item and the historical
  Project reference that open `WaveProjectsView`; only the Wave pane's
  button went.
- Asana Projects have no URL in the header: Asana project reads don't go
  through `TrackerProject` imports, only mirrored creates.
- QA 2026-11-23, real-store copy: the upgrade queues Linear Project creates
  for every unlinked live Wave in a connected repository; in
  `~/src/loopflow` that is `spend` (already queued on installed lf) and
  `engbot` (untouched since 2026-07, no `.lf/wave/engbot`). Six kept linked
  Projects are named "… — Chapter …" in Linear.
- Tracker issue observations identify their Project by its external ID. Local
  Project slugs and local IDs remain selectors for commands that accept them.
