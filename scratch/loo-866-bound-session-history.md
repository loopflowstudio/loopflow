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

Machine load average was 20+ during measurement; spreads are run-to-run. At
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
- The wide-window regression is taken for now: only manual reads (`--since
  30d`, `context_cost` from `FIRST_WEEK_START`, `bind_attribution`,
  `ops/run.rs` at 0) pay it. Measured options that narrow it and were not
  taken because 7d is already faster: widening `session_turn_events` with
  `captured_event` (30d native branch 1.0 → 0.76 s) and
  `session_events_process(lf_process_id, session_id, kind, captured_event)`
  (30d finished 1.52 → 1.22 s; 7d ~15%).

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

## Every arm has a test

`started` comes from two rows and `ended` from three sources; `windowed` names
each as an arm, and a source added only to the expression drops inputs from
every window without an error. The comment at `windowed` says so. Each arm has
a test that fails when it is removed:

| arm | test |
|---|---|
| capture `observed_at` | existing history tests |
| manifest observation | `history_windows_find_inputs_by_each_start_and_end` |
| terminal observation | same (removing the arm fails it, checked Nov 13) |
| turn `completed` | `session_events` "completed native turns in the window retain older captures" |
| Process `completed_at` | `process_completion_ends_inputs_without_exit_events` |

Not settled here: recording an input's end as one appended event would collapse
the three `ended` sources, the per-input subqueries (Exclusions) and the
finished-arm half of `windowed`. That is a store-model change beyond this Task;
it belongs in its own Task if the gate run leaves 1d below 4×.

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
- `bench.sh` reports `same` for every window, 1d ≥ 4× faster, 7d not slower.
  Open: the implement run measured 1d finished-included at 3.4× (0.51→0.15 s,
  load avg 57–80) and finished-excluded at 5.5×; the fresh-copy run at gate
  settles it. Below 4× there, the per-input `ended` evaluation (Exclusions) is
  what remains.

## Checks

- `cargo test -p loopflow --lib -- store::query_plans:: store::sqlite::session server::boundary::` — 47 pass; clippy clean (compress).
- `bench.sh /tmp/loo866/store.db` (load avg 57–80) — `same` all six windows; 1d 0.55→0.10 s, 0.51→0.15 s; 7d 0.46→0.40 s, 0.61→0.30 s; 30d 1.04→1.55 s; whole 0.93→1.23 s (implement; rerun on a fresh copy for the PR).
- `check_migrations.py`, `documented_commands::` for `docs/dev/data.md` — gate.

## Docs

- `docs/dev/data.md` query plan paragraph: plans as prepared, `LIVE_SUBSETS`
  exemption. No user-facing docs: history output is unchanged.
