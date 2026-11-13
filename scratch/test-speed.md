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
subsequent processes on that home do not. Repeated paired runs during the
integration baseline, including image copy and identity refresh in seeded times:
empty homes 1.578 / 1.554 / 1.537 s, seeded homes 0.204 / 0.182 / 0.292 s
(medians 1.554 → 0.204 s). The seed itself took 0.006 s. CLI validation remains
real. These probes used Python sqlite3 only to reproduce the Rust fixture copy;
the committed fixtures share `store/test_database.rs`.

## Integration fixture image (second win)

`tests/support::seed_store` compiles the same test-only copy helper. Account and
Task fixture registration plus transport, Machine command, Session lifecycle,
and launch-contract constructors seed before starting their first CLI.
The server-status and initial unregistered-Machine dispatch proofs use
uninitialized fixtures. They keep their absence/first-open behavior. Generic environment guards do
not seed stores, so absence/upgrade proofs keep their original starting point.

Filtered six-thread baseline: 247/247 passed, 313.090 s wall. Totals below are
**summed test seconds**, not concurrent wall time; no CI speedup is claimed.

| Group | Tests | Before local s | After local s |
|---|---:|---:|---:|
| Machine commands | 48 | 325.884 | 272.464 |
| Session launch contract | 30 | 202.828 | 159.677 |
| Session lifecycle | 41 | 330.047 | 329.797 |
| Transport (non-SSH) | 128 | 893.027 | 756.319 |

Command: `cargo nextest run -p loopflow --test transport_tests --test integration --test session_lifecycle_tests --test session_launch_contract -E '(binary(transport_tests) - test(ssh_shells::)) | test(machine_commands::) | binary(session_lifecycle_tests) | binary(session_launch_contract)' --test-threads 6`.
The first after-run exposed the Machine dispatch proof's dependence on an
uninitialized local store: the diagnostic for an initialized store differs.
Its fixture now starts uninitialized; expectations stay unchanged. The full
Machine-command rerun passed all 48 tests; the table uses that run's times.
The after-run passed the other 203 tests, including all four documentation checks.
Session lifecycle total was flat; individual process-heavy timings varied in both
directions, so this is not evidence of a whole-group speedup there.

Use inherited `LF_*` unset, `< /dev/null`, and
`LOOPFLOW_TEST_LF=$PWD/target/debug/lf`. The after-run also includes
`test(documented_commands::)` for the fixture guidance in `tests/README.md`.

## Transport timing cuts (third win)

- Replace the standalone
  `grace_expiry_refuses_bytes_before_repainting_the_terminal_screen` (62.398 s)
  with the same behavior in `transport::hosted::tests`: advance only a dropped
  viewer's fixture timestamp, drive real resume/attach requests through
  `Hosted::pass`, and check refusal before snapshot, current screen contents,
  and retention of the newer controller. No production timeout or seam changes. The host-level check passed in
  0.025 s versus 62.398 s in the old integration test.
- Delete
  `quiet_unviewed_conversations_ignore_the_obsolete_detached_limit` (5.657 s).
  `config::tests::obsolete_detached_limit_is_readable_but_not_written` covers
  the obsolete setting; `a_held_conversation_outlives_its_view` and viewer-hold
  lifecycle tests cover surviving a closed view.
- Rename the twenty-shell-lifecycles test to
  `desktop_shell_repeated_lifecycles_leave_no_owned_jobs_or_sockets` and run
  three cycles. Each retains the same full process/session/socket and settlement
  assertions; seventeen more copies assert the same behavior. Before: 49.975 s.
- Own the Machine server in the five managed-conversation cases identified below.
  Server startup stays real and the guard stops it before deleting its home.
  Focused validation: 18/18 passed in 12.756 s, including the production host
  tests, obsolete-setting config test, five guarded cases, retained-view checks,
  and three shell cycles (12.073 s versus 49.975 s for twenty).

Full non-SSH transport filter: 126/126 passed in 134.609 s wall.
Summed duration: 680.112 s, versus 756.319 s after seeding and 893.027 s
before seeding (128 tests then; one deleted, one moved to the host unit suite).
The full run's repeated-shell case took 7.541 s. After the focused and full
transport runs, `ps` showed no remaining process from this checkout's test CLI.
`cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed.

## Cleanup finding

After the two filtered transport runs, ten `target/debug/lf server run` processes
remained (five per run). `lsof` showed private `lf-7501-*` database homes, all
already removed by their fixtures. Sending SIGTERM restarted all ten through launchd. Each job's program/home
was checked against this checkout and its private fixture path, then the exact
ten jobs were unloaded with `launchctl bootout`; no other jobs were changed.
Homes recreated by their restarted servers were removed.
Fixed missing owners: the resume-JSON new-owner test, the three
`prepared_thread_json` variants, and the prepared-conversation owner/relay test.
They had started managed conversations without a `MachineServer` guard.
Both focused and full transport runs now finish without leaving fixture servers. This is a fixture
cleanup defect; no product correctness failure has been established.

## Next work

1. Store and integration image wins implemented; fmt passed.
   Clippy found a duplicated module include in the library test crate; the
   shared helper now compiles only through `test_support`, without a second
   `store` module. The image equivalence/isolation check and named bind test
   passed again (1.327 s / 0.169 s, serial); `cargo clippy --all-targets -- -D warnings` passed.
   Further focused validation: 107/107 passed in 117.684 s, including all 55
   Task graph launch tests, all 48 Machine commands, and all four quiet native
   runner variants. These are the baseline for any further runner changes.
2. Work down slow groups: transport fixed sleeps, native Linear runner's
   60 s idle wait, repeated TestRepo Git setup, redundant lifecycle checks.
   Transport candidates measured after fixture seeding: obsolete detached-limit
   integration test duplicates config parsing plus held-conversation lifetime;
   20 identical shell lifecycle iterations; a 61 s grace-expiry wait that can be
   exercised directly through the host request handler with a dropped timestamp.
   Codex quiet-runner baseline: 65.720 s (60 s is idle waiting); Claude variants
   use 1 s for the same quiet-before-turn assertion.
   Do not shorten waits that are the behavior being exercised without replacing
   their coverage. Deleted/replaced cases are listed above; no product correctness
   bug has been established.

## Limits already on this branch

`.config/nextest.toml`: a test is killed at 5 minutes, a run at 20, and five
failures stop a local run. Never run the whole workspace locally; measure with
filters (`cargo nextest run -p loopflow -E 'test(store::sqlite::durable)'`).
One cargo command at a time.
