# LOO-866: Bound Session history reads by their window

## Problem

`SqliteStore::pending_inputs` (`rust/loopflow/src/store/sqlite/sessions.rs`)
answers every Session history read: Desktop's activity feed (`lf session
history --since 7d --limit 50`, finished inputs included, via
`activity::build_snapshot` → `pending_conversation_history`), Wave detail
(`collect_recent_history`, 7d, limit 50, finished excluded), and the CLI drills.
Its cost grows with the whole store, not the window:

- the captured branch drives from `SCAN i USING INDEX session_capture_caller`:
  every capture ever recorded (17.7k), with a manifest join per row, before the
  window filter;
- the native-turn branch scans `session_turn_events` (86k rows) and groups
  every turn before `HAVING MIN(observed_at)>=since`.

`query_plans::growing_scans` does not flag either: both scans go through
partial indexes, which it exempts as "live subsets". These two are partial by
kind, not by liveness; they hold all history.

## Outcome

History reads cost what their window holds. On the Machine store copy (Nov 13,
283,822 Session events, 17,723 captures, 170k Processes), best of 5, results
identical row for row:

| read | before | after |
|---|---|---|
| 1d, finished excluded, limit 50 | 280–360 ms | 52–70 ms |
| 1d, finished included | 360–530 ms | 84–160 ms |
| 7d, finished included (Desktop activity) | 410–520 ms | 280–375 ms |
| 7d, finished excluded, limit 50 (Wave detail) | 320–390 ms | 210–350 ms |
| 30d, finished included | 550–780 ms | 815–1460 ms (regression) |
| whole store (`since=0`) | 590–900 ms | 740–1120 ms (regression) |
| Task drill (`since=0`, `?3` set) | 11–15 s | 0.01–0.37 s (Task drills below) |

Machine load average was 20+ during measurement; spreads are run-to-run. The
unscoped whole-store row is still a regression after the `?5>0` gate (Task
drills below; QA 2.5→8.7 s); Desktop's Task history and `task.context` no
longer take that path. At
7d the remaining time is per-input evaluation (`ended`, `latest`, scope
subqueries) over ~6.4k in-window inputs, not the walk.

Demo: `scratch/loo-866-bench/bench.sh <store copy>` prints `same N rows` for
every window and the before/after times; `lf session history --since 1d` on a
large store returns in tens of ms.

## Approach

Drive both branches from the window. A capture's start is
`COALESCE(manifest.observed_at, capture.observed_at)`, across two rows, so no
expression index can hold it, and captures are immutable
(`preserve_capture_update`), so no stored start column. Instead a `windowed`
CTE collects a superset of candidate captures from indexed range scans, and the
existing exact filters (`started>=?5`, `ended>=?5`, eligibility) stay as they
are. Each arm is a necessary condition:

| arm | why it is needed | index |
|---|---|---|
| exact input `?7` | `input_history` | `session_capture_key` |
| Task `?3` (time arms off) | Task drills read the Task, not all time | `session_events_task` |
| capture `observed_at>=since` | start without a manifest | `session_capture_window` (new) |
| manifest observation `>=since` → capture by receipt key | start with a manifest | `session_input_outcome_window` (new) |
| *finished:* terminal observation `>=since` | `ended` from `:terminal.json` | same |
| *finished:* `completed` turn event `>=since` → its `started` origin's `captured_event` | `ended` from turn completion | `session_turn_window` (new) |
| *finished:* Process `completed_at>=since` → captures/`started` rows naming it | `ended` from `attached.completed_at` | `processes_completed` (new), `session_events_process` |

Native turns: a `turns` CTE of turns with any turn event `>=since` (if MIN is
in the window, every event is), plus, when finished inputs are included, turns
whose `started` row's Process completed in the window. The grouping reads only
those turns.

Join order is forced with `CROSS JOIN` (window CTE first; Process before event).
Without it the planner scanned `session_capture_window` and probed the CTE, and
walked `session_process_membership` for the Process arm.
`origin.captured_event IS NOT NULL` needs `+` or the planner range-scans
`session_event_input` per session.

Prototype: `scratch/loo-866-bench/after.sql` (the query; indexes from the migration).
Indexes, with `sqlite_stat1` measured on the copy:

```sql
CREATE INDEX session_capture_window ON session_events(observed_at) WHERE kind='captured';            -- 17723 1
CREATE INDEX session_input_outcome_window ON session_events(observed_at)
    WHERE kind='observed' AND substr(receipt_key,-14) IN (':manifest.json',':terminal.json');      -- 34882 1
CREATE INDEX session_turn_window ON session_events(observed_at, kind, session_id, provider_thread, provider_turn)
    WHERE kind IN ('started','usage','completed','output');                                        -- 85815 2 1 1 1 1
CREATE INDEX processes_completed ON processes(completed_at, id) WHERE completed_at IS NOT NULL;   -- 170063 2 1
```

Data facts this relies on (copy, Nov 13): only `started` turn events carry
`captured_event` (21,184 of 21,297; `usage` 184 of 22,374; `completed` 1);
every `observed` row carries `captured_event`; no turn event has a NULL thread
or turn.

## Decisions

- One query, no per-window variant. Rust doesn't pick a plan.
- Partial indexes over `observed_at`, not a stored start column: captures are
  immutable and `ended` has three sources.
- A history read is bounded by its scope; time is one scope. A Task drill's
  window is the Task (concept review, Nov 13): Desktop's Task history
  (`RegistryQuery.taskHistory` = `session.usage` days 0) and `task.context`
  (`usage::sessions(0, task)`) read `pending_inputs` at `since=0` with only
  `?3` set. With `?3` set the time arms are off and one Task arm reads every
  `session_events` row naming the Task through `session_events_task`
  (capture, manifest, `started` origin → `captured_event`); `turns` gets the
  native `started` rows. The arms match `task_id=?3` directly:
  `pending_inputs` already binds the id `resolve_task_id` returns. The exact
  `?3` filter stays.
- Binding assigns future work only (`bind_session`): an input captured before
  the bind carries no `task_id` and is not in the Task's history, on main and
  here. `task_history_reads_every_row_naming_the_task` pins it.
- At `since<=0` the manifest, completion and Process arms are off (`?5>0`):
  the capture and turn arms already read everything.
- Measured options that narrow the wide-window cost, not taken: widening
  `session_turn_events` with `captured_event` (30d native branch 1.0 → 0.76 s)
  and `session_events_process(lf_process_id, session_id, kind, captured_event)`
  (30d finished 1.52 → 1.22 s; 7d ~15%).

## Task drills (Nov 13)

`bench.sh` now adds `since=0`, finished-included rows for the Task with the
most attributed rows and the 10th. Load avg 600–680, best of 5:

| read | main | branch |
|---|---|---|
| Task drill, 689 rows (LOO-809) | 14.6 s | 0.37 s |
| Task drill, 180 rows | 11.1 s | 0.011 s |

Warm, at load ~415: 3.8–4.5 s → 0.08–0.10 s and 3.0–4.1 s → 0.02–0.06 s.
`same` rows on every row. Plan reads `tasks` by MULTI-INDEX OR and
`session_events_task`; no history scan. (Measured before the arms and the
`eligible` `?3` filter dropped their `tasks` join; the statement now reads
no `tasks` row.)

Unscoped whole store (`bind_attribution`, `context_cost` from
`FIRST_WEEK_START`) is still slower than main. Same run, 3 tries each,
times in s:

| `since=0` | main | window arms on | `?5>0` gate (taken) |
|---|---|---|---|
| finished excluded | 8.0–10.0 | 4.4–96 | 3.1–5.2 |
| finished included | 2.8–3.0 | 6.9–8.9 | 3.9–6.0 |

The remaining gap at finished-included is the `windowed` UNION's temp
B-tree over every capture plus the `turns` join-back; main walks the index
directly. Open: whether those two callers need main's cost.

## Implemented (Nov 13)

Migration `history_window_indexes`, the `windowed`/`turns` CTEs in
`pending_inputs`, measured stats and `LIVE_SUBSETS` in `query_plans.rs`,
`session_history_reads_plan_by_window`, and
`metadata_tests::history_windows_find_inputs_by_each_start_and_end`.
Beyond the prototype:

- The bundled SQLite (3.53.2) merges `UNION` arms in output order and picked
  order-giving full scans (`SCAN session_events` by rowid, `session_turn_events`)
  over the window ranges. Arm outputs are `+col` so no index order satisfies
  the merge. The CLI `bench.sh` uses (3.51) did not do this.
- `query_plans::statements` now records the statement as prepared, not with
  values inlined: a literal `0 AND …` folds to `WHERE 0` and planned as a
  scan of an arm that never runs. `growing_scans` plans with
  `raw_query()` (parameters unbound).
- Arm aliases renamed (`completion`, `exited`, `ran`, `turn_event`) so the
  alias detector doesn't count the existing subquery `e` / `tasks t` as growing.
- Unanalyzed plans also scanned `agent_sessions s` in the final join and
  `tasks t` in the project filter; both now `CROSS JOIN` from the small side
  (`eligible`, `projects`), here and in `session_events.rs`'s event reader.

## Synced onto LOO-870 (#1916), Nov 13

Merge `65be402161`: `LIVE_SUBSETS` deleted; main's `LIVE_SUBSET_ROWS`
row-count rule and re-measured `MEASURED` taken, plus the four window indexes
with the stats above. Branch's prepared-statement tracing (`statement.sql()`)
kept; the plan query takes main's explicit NULL binding. `docs/dev/data.md`
keeps the branch paragraph and main's foreign-key text.

## Every arm has a test

`started` comes from two rows and `ended` from three sources; `windowed` names
each as an arm, and a source added only to the expression drops inputs from
every window without an error. The comment at `windowed` says so. Each arm has
a test that fails when it is removed:

| arm | test |
|---|---|
| Task (capture, manifest, `started` origin, native turn) | `task_history_reads_every_row_naming_the_task` |
| capture `observed_at` | existing history tests |
| manifest observation | `history_windows_find_inputs_by_each_start_and_end` |
| terminal observation | same (removing the arm fails it, checked Nov 13) |
| turn `completed` | `session_events` "completed native turns in the window retain older captures" |
| Process `completed_at` | `process_completion_ends_inputs_without_exit_events` |

Not settled here: recording an input's end as one appended event would collapse
the three `ended` sources, the per-input subqueries (Exclusions) and the
finished-arm half of `windowed`. That is a store-model change beyond this Task.
Open: whether to file it. The two runs disagree on 1d finished-included:
gate ~5× (1.89→0.37 s), QA ~3.5× (10.2→2.9 s), both at load avg 150+.

## Open questions

Open, not blocking Done when (record answers here):

1. Wide windows regress: 30d finished-included 0.55–0.78 → 0.8–1.5 s, whole
   store 2.5 → 8.7 s in QA. `context_cost` reads from `FIRST_WEEK_START`
   (~2 months, finished excluded) on every report. Is that cost acceptable, or
   does a wide unscoped read need main's direct walk (a second statement,
   against "One query")?
2. Recording an input's end as one appended event would remove the three
   `ended` sources and half of `windowed`. File it as its own Task or not.

## Exclusions

- Per-input cost inside the window (`ended`/`latest` subqueries for every
  in-window input, ~6.4k at 7d, 400+ "unfinished" inputs in 1d on this
  Machine). The `ordinal` partition by `ended IS NULL` needs `ended` for every
  row, so a limit cannot cut this without a stored outcome.
- Why so many inputs stay unfinished.

## Done when

- `lf session history --since 1d` and Desktop's 7d activity read only the
  window: no `SCAN` of `session_events` or `processes` in their plans,
  unanalyzed or with measured stats.
- Task drills (`taskHistory`, `task.context`) read no `SCAN` and are not slower
  than main: met (Task drills above).
- `bench.sh` reports `same` for every window, 1d ≥ 4× faster, 7d not slower.
  Gate (fresh copy, interleaved, medians of 9 at load avg 260–290): 1d
  finished-included 1.89→0.37 s (~5×), 7d finished-included 2.10→1.86 s.

## Checks

- QA run 2 (`35021f9fde`, load 150–195, `scratch/qa.html`): real Task conversation (turn, steer, `/exit`) read back through `lf session history`, `lf session usage --task` (id, `lf-…` identifier, short prefix), `lf task context` and socket `session.usage`/`task.context`, 0.01–0.4 s. Main and branch statements agree on 208 window/Task/limit combinations on that store. Fresh `VACUUM INTO` Machine copy: Task drills (ranks 1, 2, 10, 50) `same` rows, main 1.2–2.7 s → branch 0.04–0.30 s; whole store `same` 20,788 rows, finished included 2.0–3.0 → 2.5–3.3 s, finished excluded 3.0–3.5 → 3.9–4.4 s.
- Task arms + `?5>0` gate (load avg 400–680): `cargo test -p loopflow --lib -- store::query_plans:: store::sqlite::session` 38/39, the one failure the drill's selector-resolution `tasks` scan (`scratch/questions.md`); after scoping the check to the history statement, `session_history_reads_plan_by_window` passes. `task_history_reads_every_row_naming_the_task` passes. `bench.sh`: `same` on all eight rows. Clippy, CI suites: gate.
- `eligible` `?3` filter matches `task_id=?3` (input and `started` origin), no `tasks` subquery: `cargo test -p loopflow --lib -- store::query_plans:: store::sqlite::session` 39/39. Bench not rerun.
- After sync `65be402161`: `cargo test -p loopflow --lib -- store::query_plans:: store::sqlite::session server::boundary::` 51 pass; fmt and clippy clean.
- QA Nov 13 (load avg 200–250, `scratch/qa.html`): `same` all six windows on a `VACUUM INTO` copy; 1d 10.2→2.9 s and 4.5→0.6 s; 7d 6.0→5.0 s, 7.5→5.7 s; 30d 3.9→7.1 s; whole 2.5→8.7 s. Real Task conversation, steer and exit read back through branch `lf session history`; main and branch statements agree on that store. 1d finished-included still below 4×.
- Gate Nov 13 (load avg 150–290; `scripts/test.py` stopped at resource preflight on a `du` deadline with 46.9 GiB free, plan run by hand): architecture, inventory, `cargo fmt --check`, clippy, website (73), swift boundaries pass. Nextest `query_plans`, `sqlite::session*`, `server::boundary`, session_history/activity/context_cost/bind_attribution, `documented_commands`, `session_cli_tests`, `two_people`, `transport_tests`: 212/218. Six provider-startup tests missed their 30 s launch deadline; four pass on rerun, `resume_refuses_a_graph_step_and_leaves_it_running` and `session_cli_tests::terminal_titles_follow_session_rename_and_reconnect_without_provider_accounts` failed again at load 236 ("launched missing", "codex/failure failed to start"). No launch or resume path calls `pending_inputs`; CI owns them. Swift: `ThemeTests.ghosttyThemesResolveAfterYoursAndBuiltinsAndStayReadOnly` failed (theme files; no Swift in this diff); CI owns it.
- Compress after Task arms: arms match `task_id=?3` (the resolved id) instead of joining `tasks`. `cargo test -p loopflow --lib -- store::query_plans:: store::sqlite::session` 39/39 (load ~230); drill on the store copy, 689 rows, `same` as main; plan searches `session_events_task` directly. fmt clean.
- Compress after sync: no code cut; `session_turn_events` still serves the grouped turn lookup (EXPLAIN on the store copy), `session_capture_caller` still serves selector resolution.
- Hosted CI on `c6f51d6036` (Task arms, pushed): 2 pass, 15 skipped while `scratch/` holds files.
- Hosted CI on `cf1c165096`: green, with the Rust, Python and macOS suites
  deferred while `scratch/` holds files; they run once submit clears it.
- `check_migrations.py` — `history_window_indexes.sql` unmerged, additive.
- `bench.sh` on a fresh `VACUUM INTO` copy (load avg 285–290): `same` all six windows (7053/7033/1912/1902/16370/19956 rows); best-of-5 too noisy at this load, so 7d and 1d finished-included were rerun interleaved (Done when).

## Docs

- `docs/dev/data.md` query plan paragraph, written to main's row-count rule
  (example, `MEASURED` instruction) plus "as prepared, parameters unbound" and
  kind-partial indexes as history range-scanned by `observed_at`. `LIVE_SUBSETS`
  is gone from the doc. On merge, keep this paragraph and take main's
  foreign-key text after "A new hot path gets a test there".
- No user-facing docs: `lf session history` output is unchanged.
