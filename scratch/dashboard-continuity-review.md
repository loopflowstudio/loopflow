# Dashboard continuity review — October 18–19, 2026

Full record (observations, evidence links, launch details):
`08253c4dda:scratch/dashboard-continuity-review.md`. Evidence JSON remains in
`dashboard-continuity-evidence/`.

## Native review at `e08cbb102d` (synthetic Work, no provider)

Proved then, against the since-deleted split tree: a Dashboard beside the Product view,
hide/restore, a Task round trip, and an unsaved Files draft recovered by a fresh process
on the same lfdb, with zero Sessions or admissions. Not proved: same-identity unsaved
badge, live Session/shell continuity (LOO-416), duplicate promotion, stale-write
conflict, anonymous-query arrangement, CLI↔native parity (control targets `com.loopflow.mac`).

## Jack Heart's open walkthrough items (October 18)

Resolved since (golden layout, tile fill, shared `TaskHeading`/zoom, chips, jump bar,
Start offer): see `08253c4dda:scratch/dashboard-continuity-review.md`. Open: a
`lf desktop` equivalent for zoom; Jack's 0.14.7 “this needs a performance test” for
the first planning frame (below); and “queries have two purposes: what shows in teh
sidebar and waht shows in the main pane” / “probably need to look more at the sidebar
version first?”

## Planning frame performance — October 19

VACUUM copy of Jack's lfdb, 621 Tasks / 52 Waves / 55 checkouts, debug build:
4.70 s, 135 git, 8.27 MB (`5e08b6655f`) → 0.91 s, 59 git, 4.21 MB after
`b2a7d855a0` (latest Flow per Task in one pass), `c62099c2d9` (HEAD/Git layout
from disk), `f03d7cd21d` (roadmap writes each Task once). Proof tests:
`every_task_latest_flow_matches_its_own_flows`,
`planning_asks_git_once_per_live_checkout`,
`planning_runs_git_per_repository_not_per_wave`. Pre-existing failures also on
`5e08b6655f`: `desktop_commands_do_not_route_to_the_execution_machine`,
`released_work_upgrades_to_saved_intent_without_rewriting_work`.

30 leaked `ship` Waves (since-deleted test, `22e34abc54`…`418e52ba65`) await Jack's
cleanup call; nothing deleted. Open: frame bytes are mostly descriptions (1.47 MB) and
PR evidence (0.75 MB); send descriptions on demand or bound completed Tasks.
