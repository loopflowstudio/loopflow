# Open choices and run observations (LOO-995)

Stacked on LOO-938 (PR #1983); LOO-938's working notes live on its own branch.
Each choice below was made headless and is reversible before release.

## Open product choices

- **Which Project a new Task joins:** a new or moved Task joins its Wave's
  newest `started` Project, else its newest `backlog`, `planned` or `paused`
  one, else a `started` Project made in the same transaction and named for the
  Wave; creation and move print ` in Project <name>` when the Wave holds more
  than one open Project. On one store (62 live Waves, 64 Projects) the rule
  answers the old current Project in 17 of 17 Waves ("oldest started" in 15).
  No `--project` flag. Should several open Projects in a Wave ask which?
- **Projects without a tracker:** `lf wave edit-project` goes with the other
  two commands, as the brief lists it. Without Linear or Asana, a Wave keeps
  the one Project its first Task made: no `lf` command renames, adds or
  completes a Project (`docs/work.md` "Projects" states this). Is that the
  wanted no-tracker behavior, or does a local rename stay?
- **Saved KR text:** KR text in saved `projects.krs` (47 of 64 Projects on the
  measured store) stops being shown on `lf` surfaces; it stays in the leaving
  column until the contract's export and in Linear Project `content`.
- **Desktop and CLI presentation:** the standup board header loses its current
  Project line; Desktop's "Project history" becomes "Projects"; the CLI line
  drops "current" and "historical".
- **Authored Wave files:** this repository's `.lf/wave/product/GOAL.md`
  "Chapter planning" section and `.lf/wave/intelligence/metrics/*.md` are left
  alone (Wave memory and authored Wave content).
- **Project run journal (review of the built branch, 2026-11-20):** slice 5
  deletes `ProjectEventKind`, `ProjectEvent`, `HistoricalFailure` and the five
  store methods nothing calls, and marks `project_events` leaving. Passed
  over: a separate cleanup Task. Saved rows (30,536 on one store, newest
  2026-09-24) stay until the contract.
- **Which Task is in which Project:** `lf wave status` lists Projects, then
  Tasks flat. Should Tasks be listed under their Project when a Wave holds
  more than one? Not built.
- **Lock directory:** keep `~/.lf/chapter-locks` so running released clients and branch builds lock the same file.

- **Released Project selection:** the staged schema keeps `selected_project_owner_update`. On an upgraded store, a non-null `waves.current_project_id` can still prevent that Project from moving to a different Wave until the contract removes the trigger. It does not prevent moving Tasks or ending a Project.

- **Retired Project columns:** treat saved KR and metric-target JSON as opaque during store health checks. The upgrade from released Markdown still parses and converts it.

## Run observations

- **Abort reporting on the Wave save path (slice 2, 2026-11-18):** with a
  `RAISE(ABORT)` trigger on `tasks` or `task_creation_intents`,
  `lf task create --wave` and `lf task move --wave` report
  `"migration history or schema differs ... last migrated by lf 0.16.0 (91038e53d)"`
  instead of the abort; `lf task list` and `lf task edit` on the same store
  report normally. Rollback is covered by
  `store::ownership::tests::a_failed_save_into_an_empty_wave_makes_no_project`.
- **Linear OAuth contention test on stack base (slice 3, 2026-11-18):**
  `ops::linear::oauth_tests::pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers`
  fails on this branch and identically on stack base `0f00665f` (LOO-938): the
  refresh hits its 5s deadline before sending the OAuth request
  (`requests.len()` 0, expected 1).
- **Architecture and migration checks on stacked base (slice 4 & realign, 2026-11-20):**
  - `scripts/check_architecture.py` reports LOO-938's `git_tracker_*`,
    `planning_peer_*` and `planning_user*` tables missing from
    `docs/dev/reference.md`, and `plan_git` / `plan_hub` link labels missing
    symbols; inherited from LOO-938.
  - `scripts/check_migrations.py` against `origin/main` fails on missing
    `task_auto.sql` until LOO-938 rebases onto `origin/main`; `_check_staging`
    and `_check_runtime_sql` pass against the `origin/main` merge-base
    (`['plan_hub.sql', 'remove_chapters.sql']`), and
    `scripts/schema_page.py --check` passes.
- **QA on the built branch (2026-11-20):**
  - `ops::linear::planning_lookup_tests::missing_git_task_never_falls_through_to_linear`
    fails alone under nextest with `Task "FIX-404" not found`, identically on
    stack base `c2beea658a`; inherited from LOO-938.
  - `ops::linear::tests::connect_adds_an_unregistered_wave_on_main_before_linking_it`
    still asserted the deleted first Project; it now asserts none.
  - `lf wave plan <wave>` prints JSON with or without `--json`, on a TTY
    too; the base does the same.

- **QA pass 2 (07ff7bfd4, 2026-11-20):** on an Auto-Ship-approved Task,
  `lf task interrupt` stops the live Graph, then recovery's `lf --task <id> ship`
  starts a fresh one (three interrupts, three Graph runs); `lf task hold`
  stops it. Same callers on stack base `c2beea658a`; not this branch's.
