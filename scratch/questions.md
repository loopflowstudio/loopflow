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
  holds a different Project is stored beside it; reads keep the Wave's own
  record (slice 10). Only a released build sends one. LOO-1089 collapses
  them when it adds the unique index.
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
- Slice 6 (second) deleted the Project sheet, as asked ("i dont think we
  should have a wavesprojectview anymore"). It kept the inline
  `WaveProjectView` rows (Project status and tracker sync, where a pending
  create's failure shows) in the Wave pane, Backlog objective and roadmap
  card; the answer named only the sheet. Deleting those too would leave the
  header name as the only Project display and hide sync failures.
- Asana Projects have no URL in the header: Asana project reads don't go
  through `TrackerProject` imports, only mirrored creates.
- QA 2026-11-23, real-store copy: the upgrade queues Linear Project creates
  for every unlinked live Wave in a connected repository; in
  `~/src/loopflow` that is `spend` (already queued on installed lf) and
  `engbot` (untouched since 2026-07, no `.lf/wave/engbot`). Six kept linked
  Projects are named "… — Chapter …" in Linear.
- Tracker issue observations identify their Project by its external ID. Local
  Project slugs and local IDs remain selectors for commands that accept them.
- Slice 7 treats simultaneous named connects in one checkout as competing
  for the same link. They share the existing repository lifecycle lock through
  local registration and binding, then release it before inventory refresh.
- Compress after slice 7 uses a missing inventory for a skipped refresh. A successful refresh always supplies the linked Project, including when it has zero issues.
- QA run 7: a Task that never reached the tracker and stays in a removed
  Wave's Project the tracker does not hold (a completed Task; removal moves
  open ones out) gets no tracker actions: they retire on removal or on a
  reclaim that releases the link, and none are queued later. The other
  choice was a team issue with no Project, which an import could then read
  back as a move out of the removed Wave.
- Decision pass 3 chose to translate an unsent Task's old chapter destination
  to its Wave's kept Project during upgrade. A Team issue could come back
  from the tracker outside the Wave.
- Decision pass 3 chose to have explicit Wave sync attempt a pending Project
  create using the configured tracker. The connect receipt already directs
  people there after a deferred send.
- A sent membership edit keeps its former external destination in its saved
  baseline until the next send. Finding the issue there confirms the original
  move. Retrying from its observed source sends it to the kept Project.
- QA run 8: a released Wave whose kept Project arrived from another Machine
  over Git (the migration keeps the newest started one) and whose dropped
  Project was this Machine's, with its `create` queued, loses that create.
  `uncreated` skips Projects another Machine stamped, so nothing queues one:
  this Machine's translated Task creates name a Project with no external ID,
  which `remote_project` cannot resolve. The other Machine creates it in
  Linear, but a tracker repository has no Git exchange to tell this Machine.
  Open choices: let `uncreated` create any unbound Project (both creates
  carry its UUID), or have the migration keep a Project this Machine will
  create. Released lf had the same gap for Tasks it put in another Machine's
  Project; the migration widens it to Tasks it moves there.
- Concept review after slices 8–10 left the QA run 8 gap open; the answer
  "(b)" closed it in slice 11: any Machine creates an unbound Project.
- Slice 11: two Machines creating the same Asana Project at the same moment
  can still make two Asana projects (the footer read and the POST are not
  atomic). Linear's client id rules that out there.
- Sync onto main (#1973) brought `leaving_guard.rs`, which denies every
  member this branch marked leaving. Plan records still read and write
  `projects.planning_initiatives` (`WorkValue::ProjectInitiatives`, kept for
  released peers), so it went into `STILL_READ` under LOO-995. Dropping it
  from plan records instead would be a behavior change, left for the branch.
