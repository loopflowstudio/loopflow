# LOO-642 open questions and assumptions

- Resolved (Jack Heart steer, 2026-10-27): a route into `end` approves running
  that route, not a merge. `tasks.ship_lands` (draft `ship_lands`) is set only
  when `lf ship` creates a fresh approval; approvals that existed before the
  column, or came from a route into `end`, never land. Such a Task at `end`
  with an open PR ends its approval with the reason and the `lf ship` hint.
- Resolved (Jack Heart steer eb5b5f3d, 2026-10-27): explicit `lf ship` grants
  landing even to a route-only approval; automatic restart identifies itself
  with `lf ship --recover` and keeps the authority it found.
- LOO-604: Jack stopped its loop with `lf ship --stop`; its worker is active.
  Restore delivery only after liveness and handoff are clear (explicit `lf ship`).
- Answered by LOO-622 (Task brief, 2026-10-27): only its AutoShip mode
  (`lf -b task run` or `lf ship`) grants landing. Whichever of LOO-622 and
  LOO-642 lands second maps AutoShip to `lands = true` and widens "only
  `lf ship`" in AGENTS.md/orchestration/long help to "only Auto-Ship".
