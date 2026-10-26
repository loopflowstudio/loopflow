# LOO-598: idle relays and the Desktop Work feed burn CPU on a 10 GB store

The live store is `~/.lf/loopflow.db` (10.2 GB). `~/.lf/copies/45407719c5c6f0f2/` is a
same-size dev-build copy; both carry 943k processes and 2.7M session_events
(2.56M `observed`). Timings below are against the live file, read-only, at load ~220.

## Relays: `idle_session_owners` (store/sqlite/sessions.rs)

- 4.2 s user per sweep. Not the ancestor CTE (0.4 s): agent_sessions has no index
  on `transport_lf_process_id`, and the query drove from ~3,800 never-completed lf
  processes, scanning all sessions twice per candidate.
- Fix: drive from open Sessions' owners; CROSS JOIN pins the CTE seed to
  graph_processes (740 rows) instead of all processes. 44 ms user, same 23 owners.
- Default limit is 1 day, so relays sweep every 5 min; the 30% was sweeps lasting
  minutes under load. One sweeper per relay stays; at 44 ms it is not worth a
  machine-wide coordinator.

## Desktop: `lf monitor work --watch --json` → `activity::build_snapshot`

- One snapshot (`lf history --since 7d`) took 35 s; WorkActivity re-projects on every
  processes revision, so Desktop ran it back to back.
- `conversation_inputs` latest_turn subquery and the WHERE EXISTS lacked
  `session_id`, so they full-scanned session_events per captured input. Every
  `started` event's capture shares its Session (0 of 6,264 differ). Now 0.17 s.
- Uncaptured-turn branch joined each of 130k turn events to its origin; grouping the
  turn first is identical on live data (202 turns, EXCEPT empty) and 6 s → 1.9 s user,
  but 6 s sys reading fat rows. Draft `session_turn_events` covering partial index:
  0.17 s on a slim copy.

## Direction (Jack, 2026-10-26)

- Kill the cross-owner sweep: done. Each owner times its own viewers in memory and
  checks only itself (`owner_waits_for_person`); `idle_session_owners`,
  `IdleSessionOwner`, the 48 h unrecorded rule and `processes.detached_at` are gone.
- UserPromptSubmit takes the same new-steer guard as PostToolUse: done.
- Outside this Task: `__`-prefixed commands need a reason not to be a normal skill.
  `__context-block` stays. Pi callbacks could become more standard. `__repair` could
  fold into an ordinary skill. `--__cwd` could be `--cwd`. Jack didn't know about
  `telemetry-daily` (a daily cron whose `__telemetry-scorecard` step shells out to
  `scripts/lifecycle_scorecard.py`).

## Remaining

- Measure a real `lf history` / monitor frame with the branch build on a store copy.
- WorkActivity still recomputes on every processes revision; with sub-second frames
  that is acceptable, revisit only if the frame stays expensive.
