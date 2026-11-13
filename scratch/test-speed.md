# Make the Rust tests fast

Goal: as many tests as possible under 1 s, better under 100 ms. Cutting
redundant or unimportant tests is fine. Said about this work: "lets go
ballistic on these. Totally fine to cut redundant or unimportant tests. Find
way to bring as many as possiblle at <1s, even better <100ms".

## Original CI baseline

`scratch/test-timings.tsv` contains every test's median/max duration across
three merge-queue runs (2026-11-13, Linux, six threads). Keep it as the baseline;
these changes have not been remeasured on CI.

| Duration | Tests | Total seconds |
|---|---:|---:|
| under 0.1 s | 1549 | 43 |
| 0.1–1 s | 442 | 166 |
| 1–5 s | 246 | 828 |
| 5–10 s | 1055 | 7323 |
| 10–30 s | 496 | 7758 |
| over 30 s | 72 | 3592 |

About 19,700 s in all. Largest original groups included transport (1503 s,
121 tests), Task graph launch (1092 s, 56), Session lifecycle (911 s, 42),
Linear Task planning (811 s, 86), Machine commands (790 s, 48), launch contracts
(567 s, 30), local planning (553 s, 33), release locks (537 s, 13), and migration
unit tests (466 s, 112). Platform/ignored-test counts differ from the Mac runs.

## The fixed cost: migration SQL replay

Temporary timing prints in `apply_sqlite_transaction` and
`apply_set_with_purpose` measured
`store::sqlite::durable::durable_store_tests::bind_preserves_prior_work_and_usage_while_assigning_future_work`:

| Part | Local debug time |
|---|---:|
| SQL replay | 1.262277 s |
| Conversion and ledger bookkeeping | 0.007580 s |
| Entire migration loop | 1.304106 s |
| Schema validation/checksums | 0.008098 s |
| Foreign-key check | 0.000061 s |
| Persisted JSON check | 0.000121 s |
| Entire migration call | 1.313613 s |

The earlier assumption that fresh opens avoid replay was wrong:
`canonical_schema.json` avoids rebuilding the validation reference only.
The same test binary's `--list`, including its constructor, takes 0.01 s real
and less than 0.01 s CPU. All timing prints were removed.

`build.rs` now writes `test-store.db` from the connection it already migrates,
including migration versions/checksums. Fresh `cfg(test)` store opens copy it.
`tests/support::seed_store(home)` uses the same helper before fixture CLI/server
launches. Existing stores still upgrade/validate normally; raw migration tests
still replay SQL. Production does not embed or use the test image.

Every copy refreshes the local Machine identity/timestamps. The equivalence
check caught these nondeterministic seed fields before commit. It compares all
product schema objects and seed rows against runtime migration, checks distinct
Machine IDs, and proves reopening retains writes and copies stay independent.
Historical nullable migration provenance stays unset in the image.

## Local before/after (Mac, debug, 2026-11-13)

Group times are summed per-test seconds from six-thread nextest filters, not
concurrent wall time. Process-heavy timings varied; Session lifecycle was flat.

| Group | Tests before → after | Before s | After s |
|---|---:|---:|---:|
| Durable store | 20 → 20 | 45.573 | 19.359 |
| Machine commands | 48 → 48 | 325.884 | 272.464 |
| Session launch contract | 30 → 30 | 202.828 | 159.677 |
| Session lifecycle | 41 → 41 | 330.047 | 329.797 |
| Transport, non-SSH | 128 → 126 | 893.027 | 680.112 |
| Quiet native runner variants | 4 → 4 | 86.043 | 15.291 |

In the 20-test durable group, tests under 1 s increased from 0 to 14.

Named checks:

- Bind test named above: 1.521 → 0.169 s, serial nextest measurements.
- Replay-grace refusal: 62.398 s integration → 0.025 s host unit test.
- Shell lifecycle repetition: 49.975 s (20 cycles) → 7.541 s (three cycles).
- Codex quiet foreground runner: 65.720 → 3.823 s.
- Task launch/admission matrix trial: 13.345 → 15.368 s. The trial was reverted;
  no timing gain is claimed for this matrix.

Spawned CLI processes pay replay once per fresh home, not once per process.
`target/debug/lf session list --json` took 1.447 s on a fresh home, then
0.094 / 0.098 s on that home. Three paired fresh-home runs during the fixture
baseline: empty 1.578 / 1.554 / 1.537 s, seeded 0.204 / 0.182 / 0.292 s,
including copy/identity refresh (medians 1.554 → 0.204 s). Copy alone took 0.006 s.
The CLI still runs its real validation against seeded homes.

## Coverage changes

- Deleted `quiet_unviewed_conversations_ignore_the_obsolete_detached_limit`.
  The config unit test covers reading/omitting that obsolete field; retained
  conversation and viewer-hold tests cover surviving closed views.
- Replaced the standalone
  `grace_expiry_refuses_bytes_before_repainting_the_terminal_screen` with the
  same behavior under `transport::hosted::tests`. A dropped fixture timestamp
  drives real resume/attach requests through `Hosted::pass`: refusal must
  precede the fresh snapshot, the current screen must survive, and the newer
  controller must retain its claim. No production timeout/test setting added.
- Renamed the twenty-shell-lifecycles test to
  `desktop_shell_repeated_lifecycles_leave_no_owned_jobs_or_sockets` and kept
  three complete cycles, each checking process/session/socket cleanup and
  settlement. Removed seventeen copies of the same assertions.
- Codex uses the same 1 s idle observation as Claude, removing a 60 s sleep.
  All four provider/mode variants still assert zero requests before the turn
  ends, then exactly two requests and the changed title afterward. The testing
  guide no longer excludes these now-short cases from affected Linear gates.
- A trial removed twelve duplicate CLI invocations from the Task admission
  matrix. It passed, but the named check took 15.368 s versus 13.345 s and the
  full 55-case group took 423.444 s versus 317.026 s. Timing variation did not
  establish a gain, so the original matrix coverage was restored.

Bootstrap proofs remain uninitialized: server status must create no store,
and initial unregistered-Machine dispatch retains its original first-open
error. The first seeded run exposed the latter assumption; its fixture was
corrected and the full Machine group passed with unchanged expectations.

## Fixture server leak

Two full transport runs left five supervised private servers each, after their
`lf-7501-*` homes had been deleted. SIGTERM restarted all ten through launchd.
After checking each job's executable/home, the exact ten jobs were unloaded
with `launchctl bootout` and the homes they recreated were removed.

Missing `MachineServer` guards were added to the new-owner resume-JSON test,
the three `prepared_thread_json` variants, and the prepared-conversation
owner/relay test. Focused and full transport runs now leave no fixture CLI or
server process alive. This was a fixture ownership defect; no product
correctness bug was established.

## Verification and limits

Run from this checkout with inherited `LF_*` unset, `< /dev/null`, and
`LOOPFLOW_TEST_LF=$PWD/target/debug/lf` after preparing the CLI. Use nextest
filters and six threads. Never run the whole workspace locally; one Cargo
command at a time. `.config/nextest.toml` kills a test at five minutes and a run
at twenty; five failures stop a local run. No limit/CI configuration changed.

- Store/durable/frontier and store/viewer caller filters: 93/93 passed, 14.595 s;
  image/bind recheck after module consolidation: 2/2 passed, 1.498 s.
- Four integration group baseline: 247/247 passed, 313.090 s. Seeded run:
  250/251 passed; the sole failure was the initial-Machine assumption above.
  Corrected full Machine group, Task graph group and four native variants:
  107/107 passed, 117.684 s. All four documentation checks passed.
- Transport host/model/config and changed public cases: 18/18 passed, 12.756 s.
  Full non-SSH transport: 126/126 passed, 134.609 s; no leftover fixture processes.
- Quiet native variants and documentation: 8/8 passed, 7.702 s.
- Task graph/image trial filter: 56/56 passed, 72.621 s. The original matrix
  was restored afterward; its unchanged coverage passed in the earlier full
  107-case run. Eight holder processes briefly outlived the trial suite and had
  exited by the post-clippy process check.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` passed before each
  completed local speedup commit. Final formatting/import cleanup lint passed.
  Linux/SSH checks remain with CI; the SSH suite was not changed.

## What remains

1. Remeasure affected filters on Linux before projecting a CI-wide saving.
2. Profile repeated `TestRepo` setup and remaining fresh-home constructors
   (local planning, global commands, served reads, release locks). Extend
   `support::seed_store` only to ordinary fixtures, keeping bootstrap/upgrade
   starting states intact.
3. Transport's remaining 30–36 s checks exercise actual heartbeat, reconnect,
   and pending-turn lifetimes. Move a check to direct host/model coverage only
   when it still exercises the failure people would notice; keep one real
   protocol proof per behavior.
4. Raw migration/prefix tests still replay SQL intentionally. Prepared prefix
   images might help their setup, but must retain real upgrade coverage.

Concurrent repository activity: at 12:14:11 another process made/pushed
`2fe6b81d27` (`lf pr open: prepare branch`) while these checks were active.
Read-only `gh pr view` showed open PR #1920 at that commit. This run did not
push or create a PR; it retained the external commit and continued locally.
