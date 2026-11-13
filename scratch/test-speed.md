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

## Local profiling and first fix (2026-11-13, Mac, debug)

Temporary timing prints (removed) in `apply_sqlite_transaction` and
`apply_set_with_purpose`, using
`store::sqlite::durable::durable_store_tests::bind_preserves_prior_work_and_usage_while_assigning_future_work`:

| Part | Time |
|---|---:|
| SQL replay | 1.262277 s |
| Conversion and ledger bookkeeping | 0.007580 s |
| Migration loop total | 1.304106 s |
| Schema validation/checksums | 0.008098 s |
| Foreign key check | 0.000061 s |
| Persisted JSON check | 0.000121 s |
| Migration call total | 1.313613 s |

The earlier claim that fresh opens avoid migration replay was wrong:
`canonical_schema.json` avoids rebuilding the *validation reference* only.
`--list` on the same test binary (including its pre-main constructor) takes
0.01 s real, below the profiler's 0.01 s CPU resolution.

Build now writes `test-store.db` from its already-migrated catalog connection,
including migration versions/checksums. Only fresh stores in `cfg(test)` copy
it; existing stores still upgrade/validate. Raw migration tests still replay
SQL. The image's nullable historical provenance is unset; it claims no runtime
migration application. Copying refreshes the local Machine identity/timestamps (the comparison caught
these nondeterministic seed fields); all other seed values stay identical.
A regression compares every product schema object and
seed row with actual runtime migration, and checks independent copies and reopen.

Baseline: `cargo nextest run -p loopflow --lib -E
'test(store::sqlite::durable::durable_store_tests::)' --test-threads 6`:
20 passed, 8.394 s wall; named bind test 1.521 s alone / 1.674 s in group.
Initial copied-image run: named bind test 0.298 s, mixed with frontier tests.
After: same 20 names total 19.359 s (summed per-test durations) versus
45.573 s before; bind 0.285 s in the mixed six-thread validation run.
The full selected store/frontier/viewer validation: 93/93 passed, 14.595 s.
The equivalence test found the Machine identity/timestamp difference before
commit; refreshing those fields fixed it. Raw migration replay remains covered.
One initial validation failed only this new image check; rerun above is green.
The initial viewer run spent 66 s per resume case preparing a CLI; passing
`LOOPFLOW_TEST_LF=$PWD/target/debug/lf` reduced them to 6.3–6.6 s.
`cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

## Spawned CLI baseline

Against one fresh private `LF_HOME`, `target/debug/lf session list --json`:
1.447 s first open, 0.094 s second, 0.098 s third. Each new home pays replay;
subsequent processes on that home do not. Fixture images should remove the
first open's replay while retaining real CLI validation on subsequent opens.

## Next work

1. Store image win validated and ready to commit; continue with integration.
2. Seed integration fixture homes from the same image; measure filtered
   transport and integration groups before/after.
3. Work down slow groups: transport fixed sleeps, native Linear runner's
   60 s idle wait, repeated TestRepo Git setup, redundant lifecycle checks.
   Do not shorten waits that are the behavior being exercised without replacing
   their coverage. No tests deleted yet; no product bug identified yet.

## Limits already on this branch

`.config/nextest.toml`: a test is killed at 5 minutes, a run at 20, and five
failures stop a local run. Never run the whole workspace locally; measure with
filters (`cargo nextest run -p loopflow -E 'test(store::sqlite::durable)'`).
One cargo command at a time.
