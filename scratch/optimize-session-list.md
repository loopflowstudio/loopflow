# Session-list optimization measurements — LOO-822

**Better: the kept gains survive compress and sync onto main.** Fresh comparison of
`00851d9c6` → `b8fd92ab6b`, five alternating pairs per workload: default Session list
**24.5% lower**, served **70.7% lower**, Wave list **43.9% lower**, and network-denied
Task status **85.6% lower**. Work selection is **55.9% lower**; board has **no
measurable difference**. All 60 timed results and their warm-ups match complete JSON,
with zero command or validation errors. **No fields differ; no main-merge exceptions are
needed.**

Final measurement, 2026-11-11. **Median [minimum–maximum] milliseconds; n=5 per side.**

| Workload | Baseline | HEAD | Verdict |
|---|---:|---:|---|
| Default Session list | 848.7 [807.8–949.1] | 640.4 [506.0–685.3] | Better: 24.5% lower |
| Warm served Session list | 893.7 [868.6–968.9] | 262.0 [233.7–278.9] | Better: 70.7% lower |
| Wave list | 445.8 [437.8–519.6] | 249.9 [244.1–255.5] | Better: 43.9% lower |
| Task status LOO-822, network denied | 9432.2 [8136.4–11623.1] | 1362.0 [1267.0–1933.4] | Better: 85.6% lower |
| Board | 1047.4 [874.1–1173.4] | 1022.2 [998.1–1147.8] | No measurable difference |
| Work selection | 1126.5 [1096.9–1154.4] | 496.5 [455.6–540.8] | Better: 55.9% lower |

This confirms the relative gains, not the old absolute latencies: the earlier CLI
579→436 ms, served 775→211 ms, Wave 441→247 ms and Task 7.3 s→1.24 s came from earlier
cohorts. Here CLI is 849→640 ms, served 894→262 ms, Wave 446→250 ms and Task 9.43 s→1.36
s. The fresh store and shared Machine differ; do not combine cohorts into a speedup. All
five improving workloads have non-overlapping ranges. The original 18–75 s Session-list
stall remains unexplained.

## Final measurement details

Both exact commit exports were rebuilt in release mode with Rust 1.98.1 on macOS 26.0.1
arm64, then measured through the unchanged five-file harness. A fresh read-only online
backup supplied the private snapshot: 14,804 Sessions, 237,607 events, 160,530 Processes
and 1,099 Tasks. Every measured lf command used /tmp/loo822-final-measure, never the
real Machine store. Both migrated copies were checkpointed to DELETE mode, with no WAL;
all 79 application-table digests match except schema_migrations. Baseline has none of
the four optimization indexes; HEAD has exactly those four. Clock, credential-free
provider copies, executable/home paths and warm-up policy match across sides. Store,
harness and protected loo-822.md hashes remain unchanged.

The timed region includes command execution and JSON collection; served timing covers
the requested checkpoint through its answer. Answers contain 100 default Sessions and
372 served Sessions (zero raw rows), seven Waves, one Task with 90 unique Sessions, 182
board Tasks and 25 Work members. Session titles/directories and Task titles match the
snapshot. One-row calibration (281.1–400.6 ms baseline, 276.4–293.1 ms HEAD) is
distinguishable from the calibration’s 100-row reads (573.0–584.5 and 424.0–424.5 ms).
All 12 preflight answers, eight calibration answers and 14 profile answers also
validate; profiles match the timed full JSON. No fields or array ordering were removed.
The ten timed board calls emit 40 existing unknown-crons warning lines.

Fresh, excluded SQLite 3.53.2 profiles confirm the kept plans. The default Session
summary falls from 196,961 full-scan / 1,999,467 VM steps to 4,129 / 85,802; its
statement time is 233.6→17.0 ms. The served checkpoint falls from 764,212 / 7,639,765 to
4,401 / 267,049, with statement time 891.8→111.6 ms. These are
store/sqlite/sessions.rs::session_summaries_in using task_work.rs::session_tasks; both
Task indexes appear in HEAD’s bound plans. Served figures use the second summary
execution, inside the timed checkpoint, excluding the initial frame.

Wave live-agent inventory falls from 160,529 full-scan / 675,261 VM steps to 60 / 2,205,
and 194.7→0.4 ms, through processes.rs::agent_processes and agent_process_unfinished.
Task Process membership falls from 175,599 / 7,655,559 to 15,070 / 279,500, and
350.9→9.1 ms, through task_work.rs::lf_process_ids and processes_cwd equality/range
probes. Work selection’s summary falls from 764,212 / 7,642,754 to 4,401 / 270,038, and
934.3→115.1 ms. Board’s leading substantive query counters remain unchanged.

The removed default-list statement work is 216.6 ms, close to the paired command’s 208.2
ms median reduction. In the separate profiled baselines, that statement occupies 26.4%
of CLI time; the served summary occupies 80.3%, the Wave inventory 40.0%, and
Work-selection summary 55.7%. Removing each entire piece caps improvement at those
shares in those runs. Profiled served, Wave and Work paths fall 74.7%, 17.1% and 50.9%,
within those ceilings. The profiled CLI falls 39.4%, which the summary alone cannot
explain: other elapsed costs and instrumentation differ between those two runs. That
profiled CLI wall-time difference is inconclusive and is not used as the speed estimate.
The uninstrumented five-pair claim is supported by the matching outputs, separated
ranges and independently observed 216.6 ms of removed query work; profiler times do not
exactly decompose its latency.

Task-status native samples retain the same attribution: Keychain reads fall from
4,025/7,094 (56.7%) main-thread samples to 37/841 (4.4%); check_scope falls from
1,613/7,094 (22.7%) to 4/841 (0.5%). The source paths are waves.rs::task_detail →
human_session::listed → LaunchCapacity and linear_observe::check_scope →
planning_client::repository_scope. Capacity, scope and SQLite occupy 89.4% of baseline
samples counted as a union, without nested double-counting. The sampled commands fall
85.1% (9,869.9→1,473.0 ms), below that approximate ceiling. Stack shares are not a
wall-time decomposition. HEAD’s next limiter remains RepoId::discover during planning
field delivery: 303/841 samples (36.0%); sqlite3_step accounts for 207/841 (24.6%).
These overlapping profile views are not added.

Lock scope in the fresh profiles: neither Session-list path issues BEGIN IMMEDIATE. Each
network-denied Task-status profile does so once for linear_team_checks, then rolls back;
acquisition takes 0.0026 ms before and 0.0025 ms after. Those Task reads can take the
writer lock, but these runs show no long wait and do not reproduce the Session-list
stall.

These are complete warm local paths, not microbenchmarks. Task status, Wave list, board
and Work selection run with network denied; this does not measure successful planning
exchange. Credential-free provider copies leave native Keychain fallback costs unlike
some normal logins. OS processes and Git/config facts remain live. Timed one-minute
Machine load was 12.3–18.6; other builds and tests were observed. The snapshot is
sealed, not the Machine. No synthetic writer or load was introduced. No production
source changed in this final measurement; no implementation revert is indicated. All
benchmark servers and samplers exited.

Receipts: [all trials and sealed-input checks](session-list-measure/final-results.json), [excluded SQL and stack profiles](session-list-measure/final-profiles.json), [measurement ledger](measure.html#final). Raw answers and stores remain private.

## Iteration 1 verdict

**Better: keep `d9e3c1f3b`.** Indexed reverse Task probes cut the default
100-row CLI median from **579.0 to 436.3 ms (24.7% lower)** and the unpaged
371-row served median from **775.3 to 210.7 ms (72.8% lower)**. Five alternating
pairs per workload; ranges do not overlap; full JSON matches; zero command or
validation errors. Both provisional targets (15% CLI, 25% served) are met.

**No measurable additional benefit: reject `f8b742d8c`.** Its CLI, served,
board and Work-selection ranges overlap those of `d9e3c1f3b`. The production
and test changes from that commit are restored to `d9e3c1f3b`, including the
extra indexes in this Task’s migration. The commit and all trial receipts stay
in history. No synthetic writer or load was introduced.

The original 18–75 s Session-list stall remains unexplained. These measurements
do not establish a fix for that stall.

## Iteration 1 receipts

Complete comparisons, frozen-input limits, excluded calls, SQL attribution and
checks: `0f39f7be21:scratch/optimize-session-list.md`. Machine-readable artifacts
remain in `session-list-measure/`, linked by the iteration 2 report below.
The original 18–75 s stall was not reproduced; no normal read acquired a writer
lock or retried one. Process inventory was the next measured limiter.

## Recorded direction

Jack Heart: "Lets take a look at all the queries that happen during lf session list and look for places we get stuck behind and a write lock or do a table scan"

Jack Heart: "you can also do a cursory scan of other common planning queries and optimzie those as well"

## Original baseline — historical receipts

Complete original baseline and implementation receipts:
`2667788c600d29471fd880e43005174d097fee19:scratch/optimize-session-list.md`.
Machine-readable SQL, samples, validation, and scripts remain in
`session-list-measure/`; private data stays in `/tmp/loo822-measure`.

The original 2026-11-11 run on `00851d9c6` measured CLI 788 ms
(701–863), served 963 ms (854–1116), five samples each. No call approached
18 s. The real Pi writer experiment observed external commits in every
window, but no Session-event/usage revisions: event-stream contention remains
unmeasured. Pi's answer was incomplete; its detached processes were stopped.
No normal measured read acquired a writer lock or retried a lock failure.
The unreproduced stall still needs stacks captured while it is happening.

## Historical iterations 2 and 3

Full measurements, invalid first cohort, attribution, bounds and receipts:
`71beb6050d78c67d731619b50f46ac0495b24efc:scratch/optimize-session-list.md`.
The sealed snapshots, warm-ups and per-call JSON remain in
`session-list-measure/iteration{2,3}-*.json`; the final confirmation above
supersedes their latency estimates. No board speedup was attributed to the
indexes. Event-stream contention and the original 18–75 s stall remain
unmeasured. Repeated Git discovery remains the next measured limiter.

## Kept implementation and verification

`8d47895760` shares the relocation probe and stores repository-match booleans
within each scope check. It retains the four measured indexes, released-schema
reads, lexical/relocation behavior, upgrade comparisons, query-plan assertions
and VM bounds. `2667788c6` reverted `f8b742d8c`; its two extra indexes are absent.
No further optimization was implemented for the Git-discovery cost above.

- Pre-sync combined focused Rust check: 45 passed (Task membership, live
  Process inventory, Session operations and planning scope/adoption).
- `cargo fmt`; `git diff --check`; `cargo clippy --all-targets -- -D warnings`:
  passed before sync. Existing oversized unwind-table linker warning only.
- Exact commands and earlier focused checks:
  `8d47895760:scratch/optimize-session-list.md`. Gate owns affected suites on
  the merged tree; documentation reconciliation does not rerun these checks.
