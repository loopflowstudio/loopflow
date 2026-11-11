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

## Iteration 2 measurement — sealed private stores (2026-11-11)

**Better: keep `761c28056` over `2667788c6`.** Claim tested: “Indexing Process
inventories reduces local planning latency beyond run-to-run noise without
changing any answers.” Five alternating pairs per workload, reversing order
each pair. All entries below are **median [minimum–maximum] milliseconds,
n=5 per side**. Zero command or validation errors; every timed answer and
warm-up matches its workload’s complete JSON digest.

| Workload | Before | After | Verdict |
|---|---:|---:|---|
| Wave list | 441.4 [410.9–454.4] | 246.5 [246.2–261.4] | Better: 44.2% lower |
| Task status LOO-822, network denied | 7373.0 [7058.6–7470.8] | 6864.7 [6606.6–7042.6] | Better: 6.9% lower |
| Default Session list | 434.9 [425.3–443.1] | 420.3 [401.2–431.5] | No measurable difference |
| Warm served Session list | 195.0 [188.8–215.1] | 202.6 [188.0–229.9] | No measurable difference |
| Board | 876.4 [865.7–899.0] | 853.1 [833.7–863.2] | Inconclusive |
| Work selection | 457.2 [442.4–476.0] | 435.9 [431.8–455.4] | No measurable difference |

Board’s 2.7% separation is unexplained: neither new index appears in its bound
plans, and all substantive statement plans and work counters match. Only the
schema-inventory query sees the two additional indexes. No board speedup is
attributed to the change. Task status’s ranges separate by only 16 ms; this is
a five-pair result on this snapshot, not a general latency guarantee.

### Sealed inputs, attribution and limits

Complete iteration-2 setup, excluded trials, stack counts and check receipts:
`a33b0721fb:scratch/optimize-session-list.md`. The initial 60-call cohort was
invalid: a backup connection left the kept Task indexes in a WAL, and copying
only the main database omitted them. Retained trials use checkpointed DELETE-mode
stores; immutable reads verified all 78 application tables match. The backup
contains 14,759 Sessions, 236,791 events, 160,314 Processes and 1,099 Tasks.

SQLite 3.53.2 bound plans and work counters, fullscan / VM steps per execution:

| Statement | Before | After |
|---|---:|---:|
| Unfinished-agent inventory | 160,313 / 674,239 | 58 / 2,133 |
| Task Process membership | 175,320 / 7,641,629 | 15,007 / 274,770 |
| Work-selection Session summary | 4,401 / 270,064 | 4,401 / 270,064 |

The unfinished-agent partial index and cwd equality/range indexes remove
retained-history scans. Session membership and relocation scans remain.
Warm native samples found repeated Keychain reads at 62.1% of the after
main-thread samples and planning scope checks at 22.6%; iteration 3 follows
those costs. These samples do not explain the original 18–75 s stall.

All 60 retained timed calls and warm-ups match complete JSON, with zero errors.
Private stores and provider copies omit credentials; network is denied and
OS/Git/config facts remain ambient. No board speedup is attributed to the
change: its substantive SQL plans and counters match. All samplers and servers
exited. Raw captures stay in `/tmp/loo822-process-measure`; `unsealed/` retains
the invalid cohort.

- [Every trial](measure.html#iteration2-trials),
  [samples, hashes and excluded cohort](session-list-measure/iteration2-results.json),
  [bound plans and counters](session-list-measure/iteration2-profiles.json),
  [validation and stacks](session-list-measure/iteration2-checks.json).

## Iteration 3 measurement — repeated reads (2026-11-11)

**Better: keep both commits.** Claim tested: “Each repeated-read removal reduces
network-denied Task-status latency beyond run-to-run noise, with identical full
JSON.” The primary is the complete warm CLI command, including output collection.
Each comparison has five alternating pairs, reversing order each pair. Values
are **median [minimum–maximum] milliseconds, n=5 per side**.

| Change / workload | Before | After | Verdict |
|---|---:|---:|---|
| Capacity: Task status, network denied | 7285.6 [6957.5–8583.1] | 2982.7 [2733.3–3222.4] | Better: 59.1% lower |
| Capacity: default Session list | 441.7 [440.4–451.8] | 440.6 [427.9–449.7] | No measurable difference |
| Capacity: Wave list | 277.7 [264.4–281.6] | 269.0 [266.1–271.9] | No measurable difference |
| Capacity: board | 859.5 [835.9–878.7] | 870.5 [849.1–899.4] | No measurable difference |
| Scope: Task status, network denied | 3225.8 [3155.9–3311.5] | 1243.1 [1230.8–1269.2] | Better: 61.5% lower |
| Scope: default Session list | 440.5 [431.2–450.6] | 440.4 [432.8–450.4] | No measurable difference |
| Scope: Wave list | 274.6 [268.2–281.4] | 269.7 [262.7–271.7] | No measurable difference |
| Scope: board | 852.2 [828.3–889.1] | 835.5 [826.7–886.7] | No measurable difference |

Capacity compares `0f39f7be21` → `d1f1caa3f7`; scope compares the kept
`d1f1caa3f7` → `a835f03e6f`. Neither requires a revert. The two capacity-side
medians come from different cohorts; no combined paired speedup is claimed.

### Frozen inputs and correctness

Iteration 3 reuses iteration 2's checkpointed snapshot. All three database
files had the same SHA-256, no WAL, and matching digests for all 78 tables;
the five sealed harness hashes matched. Private homes and transcript copies,
a frozen wall clock and excluded warm-ups are shared across sides. Exact
commit exports used release builds, Rust 1.98.1 and macOS 26.0.1 arm64.
Calibration distinguishes one row at 265.8–269.3 ms from the default 100 rows
at 435.5–448.1 ms. Reproduction details and private paths are in the history
reference above and the linked artifacts below.

All 80 timed calls and warm-ups match complete JSON per workload, as do the
60 separate spawn-count calls, three native samples and four wall/SQL
attribution calls. Zero command or validation errors. Answers contain one
Task with 84 unique Sessions, 100 default Sessions, seven Waves and 183 board
Tasks. Task Session titles/cwds match the sealed store; the harness checks
Session fields, unique IDs, usage and Task titles. No fields are normalized
or omitted. The 20 timed board calls emit 80 existing unknown-`crons` warnings.

These are warm local reads with network denied, not successful planning
exchange or online authentication. Private homes omit credential files;
Keychain fallback costs can differ from normal logins. OS processes and
Git/config files remain ambient. Timed one-minute Machine load ranges
10.0–26.6. The original 18–75 s Session-list stall remains unreproduced.

### What changed in a run

The native baseline sample has **3,318/5,337 (62.2%)** main-thread samples in
`read_claude_keychain_blob_with_security`; after capacity reuse, **49/2,243
(2.2%)**. Separate DYLD-interposed runs count successful `security` child PIDs,
never arguments, credential values or command output. Each cell below is the
same count in all five attribution calls:

| Call | Before | Capacity | Scope |
|---|---:|---:|---:|
| Task status | 336 | 4 | 4 |
| Default Session list | 4 | 4 | 4 |
| Wave list | 0 | 0 | 0 |
| Board | 0 | 0 | 0 |

The Task still returns all 84 Sessions. Removing 98.8% of subprocesses from
a path occupying about 62.2% of baseline samples gives an approximate 61.5%
latency ceiling, consistent with the observed 59.1% reduction. Stack shares
cover captured main-thread samples, not the complete elapsed command.

After capacity reuse, planning scope checks account for 50.9% of captured
main-thread samples. This fraction alone does not explain the scope cohort’s
61.5% reduction, so separate release attribution builds add wall timers at
`planning_client::repository_scope` and `linear_observe::check_scope`.
They record **1,154 → 40 scope resolutions**, taking **1,508.7 → 16.7 ms**.
The complete check takes **1,560.7 → 43.2 ms** in commands lasting
**2,662.7 → 1,125.1 ms**. The check saves **1,517.6 ms** of the command’s
**1,537.6 ms** reduction (98.7%). Its baseline share is 58.6%, giving at most
58.6% lower latency (2.42× speed); the profiled path’s 57.7% reduction fits.
Nested scope time is not added. These bounds belong to those instrumented
runs; they do not numerically decompose the 61.5% measured in the earlier,
uninstrumented cohort.

Bound-SQL attribution uses the same SQLite 3.53.2 callback as iteration 2.
Both scope sides execute **3,812 statements**, with matching leading work
counters. The repeated 84 Session summaries take 25.6 → 25.7 ms; no SQL gain
is attributed to scope caching. Counter, sampler and SQL instrumentation are
absent from the five-pair timing cohorts.

### Next limiter and checks

The final native sample points to repeated Git discovery during planning field
delivery: **204/578 main-thread samples (35.3%)** in `RepoId::discover`.
A separate wall-timer run records **28 calls taking 318.9 ms**, **28.3%** of
its 1,125.1 ms command. Path:
`linear_observe::sync_scope` → `planning_delivery::deliver_fields` →
`PlanningClient::for_repository` → `linear::resolve_repository_context` →
`RepoId::discover` → `git remote get-url origin`.
Removing all of that work would cap that run’s improvement at 28.3% lower
latency, about 1.40× speed. This is the next measurement target, not an
implementation made here.

SQLite is another remaining cost: 29.8% of final main-thread samples are in
`sqlite3_step`; the final SQL trace’s leading Process query takes 112.7 ms
across five executions. Profile measurements overlap and are not added.

Three uninstrumented and two attribution release builds passed, as did
`cargo fmt --check` and `git diff --check`. Frozen store and harness hashes
were unchanged. All samplers and commands exited.

- [Every trial and warm-up](measure.html#iteration3-trials),
  [timings, spawn counts and hashes](session-list-measure/iteration3-results.json),
  [stack attribution and checks](session-list-measure/iteration3-checks.json),
  [leading SQL and counters](session-list-measure/iteration3-profiles.json).
  All 151 calls and their excluded warm-ups are retained; raw JSON, stores and
  captures stay in the private `/tmp/loo822-reads-measure` directory.

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
