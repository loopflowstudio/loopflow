# Make the Rust tests fast

Goal: as many tests as possible under 1 s, better under 100 ms. Cutting
redundant or unimportant tests is fine. Said about this work: "lets go
ballistic on these. Totally fine to cut redundant or unimportant tests. Find
way to bring as many as possiblle at <1s, even better <100ms".

## What three merge-queue runs showed (2026-11-13, Linux, 6 threads)

`scratch/test-timings.tsv` has every test: median and max seconds, binary, name.

| Duration | Tests | Total seconds |
|---|---|---|
| under 0.1 s | 1549 | 43 |
| 0.1–1 s | 442 | 166 |
| 1–5 s | 246 | 828 |
| 5–10 s | 1055 | 7323 |
| 10–30 s | 496 | 7758 |
| over 30 s | 72 | 3592 |

About 19,700 s in all. The 5–10 s band is one fixed cost: modules such as
`store::sqlite::durable`, `session_events`, `task_work`, `harness::agent_process`
have a per-test minimum near 5 s and a median near 6.3 s.

## What a local run showed (this Mac, debug test binary)

`store::sqlite::durable::durable_store_tests::bind_preserves_prior_work_and_usage_while_assigning_future_work`
run alone from the built binary: 1.50 s real, 1.33 s user. A second test in the
same module: 1.42 s real, 1.33 s user. So a fresh store costs about 1.3 s of CPU
here before the test does anything, and about 5 s on a CI runner. Not yet
located: which part of `SqliteStore::open_ephemeral` (schema build, migration
ledger, `validate_foreign_keys`, `validate_persisted_json`, the test ctor in
`support`) the time is in. `build.rs` already generates the schema so that
opening does not replay 158 migrations, and `libsqlite3-sys` already builds at
opt-level 2 in dev and test profiles.

Integration tests spawn `lf` against a fresh `LF_HOME`, so each spawned process
that opens a store likely pays the same cost again; tests at 10–40 s may be
that cost several times over. Not measured.

Largest groups by total seconds: `transport_tests` 1503 (121 tests),
`task_graph_launch_tests` 1092 (56), `session_lifecycle_tests` 911 (42),
`ops::linear::task_planning_tests` 811 (86), `machine_commands` 790 (48),
`session_launch_contract` 567 (30), `local_planning` 553 (33),
`release_lock_tests` 537 (13), `store::migrations::tests` 466 (112).

## Limits already on this branch

`.config/nextest.toml`: a test is killed at 5 minutes, a run at 20, and five
failures stop a local run. Never run the whole workspace locally; measure with
filters (`cargo nextest run -p loopflow -E 'test(store::sqlite::durable)'`).
One cargo command at a time.
