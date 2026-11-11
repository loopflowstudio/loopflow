# Session-list optimization measurements — LOO-822

**Iteration 3: better; keep `d1f1caa3f7` and `a835f03e6f`.** On the sealed
iteration-2 snapshot, one capacity read cuts network-denied Task-status median
**59.1%**; resolving scope once per repository cuts another **61.5%** against
the kept capacity change. Five alternating pairs each; all full JSON matches,
zero errors, and no measurable guard regressions. Next limiter: repeated Git
repository discovery during planning field delivery.

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

## Iteration 2 — Process inventory (2026-11-11)

Replace the Process scans with an unfinished-agent partial index ordered by
id, and cwd equality/range probes from the selected Task. Keep relocation
receipts, history and attachment membership, ordering and all returned fields.
Reuse `session_list_indexes.sql`. Session flags and primary-Session design
are outside this iteration.

Delete — do not maintain: the `lf_process_ids` full Process join using
`instr(cwd, root)`; its behavior moves to binary lexical cwd probes. No data,
API or existing behavior test is deleted. `process_membership` remains for
single-Process and bounded unfinished-Process reads.

Focused coverage: `store::sqlite::processes` live inventory and
`store::sqlite::task_work` membership, upgrade plans and VM-step bounds.
Direct callers: `harness::agent_process`, Task status and Project adoption.
Gate owns the remaining affected suites, `two_people`, `transport_tests`,
and frozen full-JSON/wall-time comparisons before a speedup verdict.

- `cargo test -p loopflow --lib -- store::sqlite::task_work::tests::`: 13 passed, including cwd equality/range plans, VM bound and lexical/relocation equivalence.
- `cargo test -p loopflow --lib -- store::sqlite::processes::attachment_tests::live_agent_inventory_upgrade_skips_other_processes store::sqlite::processes::attachment_tests::detached_and_replaced_agents_remain_in_the_process_inventory`: both passed; the upgrade test passed on rerun after adding its missing fixture cwd. Checks compare every selected value across migration and bound live-index scans to two steps. LF variables unset and stdin closed for all tests.
- `uv run python scripts/check_migrations.py`; `cargo fmt`: passed. The test linker repeated the existing oversized `__eh_frame` warning; compilation and tests passed.
- `cargo clippy --all-targets -- -D warnings`; `git diff --check`: passed.


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

### Setup correction and retained trials

The first 60-call cohort is **invalid**, despite matching answers. Python’s
SQLite connection context manager commits but does not close the connection.
The preparation backup left the baseline’s already-kept Task indexes in a
650 MB WAL. `activate` copied only the main database. Immutable inspection
showed no Task indexes in that file, while a normal connection saw both.
Commands that open the registry for migration repaired this; Work selection’s
read-only path did not. Its apparent 45.3% gain therefore included iteration 1.

Both private databases were checkpointed and sealed in DELETE journal mode.
`immutable=1` reads verified their standalone indexes and all 78 application
tables: every row digest matches, with only `schema_migrations` differing.
The preparation script now closes connections and seals the destination before
copying. The timing code (`compare.py`, `planning_compare.py`, `run.py`, clock
shim and iteration-2 plan) is unchanged; its hashes were checked again after
all runs. All six workloads were rerun, five pairs each. The ledger retains
both cohorts, all warm-ups, calibration, preparation and profiling calls.

Fresh read-only online Machine-store backup: **14,759 Sessions, 236,791 events,
12,983 activity rows, 160,314 Processes, 1,099 Tasks and six accounts**. Every
benchmark command used `/tmp/loo822-process-measure`, never the live store.
The private provider transcript copies from iteration 1 were reused without
credentials. Wall time stayed fixed at the epoch in the manifest; monotonic
clocks supplied elapsed time. No artificial load or writer was introduced.

Both exact commit archives were built with `cargo build --release -p loopflow
--bin lf`, Rust/Cargo 1.98.1, macOS 26.0.1 arm64, 16 logical CPUs, 128 GiB RAM.
Both attribution binaries use the same release flags and the existing
`trace_bound.rs` callback. All four builds finished before retained timing.
Executable and home paths are identical across sides. No output field was
removed or normalized. The exported builds have development provenance and no
Git metadata; commit mapping and binary hashes are in the receipt.

The corrected calibration distinguished a one-row list (248.3–253.3 ms) from
the default list (423.0–441.3 ms), two alternating pairs. Timed work
includes command execution and output collection; served timing includes the
checkpoint’s reading. Clone/reset, migrations and validation are outside it.
Every CLI has an excluded warm-up; every server has an initial subscription.
All private servers were stopped after their reads.

The 60 retained timed calls return 7 Waves, one Task with 84 Session rows,
100 default Sessions, 372 served Sessions, 183 board Tasks and 25 Work members.
Session title/cwd and Task titles match the private store; IDs are unique and
turn states/usage validate. Full JSON also matches all 12 attribution reads
and both final stack samples. The ten board reads emit 40 existing unknown-
`crons` warning lines from another checkout. Network-denied planning measures
the local answer, not successful exchange; expected blocked network attempts
are not counted as successful work. OS processes, Git/config files and legacy
absolute transcript paths remain ambient; one-minute Machine load ranged from
7.9 to 11.0 in the retained cohort and is recorded per trial.

### Bundled-SQLite attribution and the latency limit

SQLite **3.53.2**, actual bound plans, monotonic statement timing, separate runs.
Counters are per execution, **fullscan / VM steps**:

| Statement | Before | After |
|---|---:|---:|
| Unfinished-agent inventory | 160,313 / 674,239 | 58 / 2,133 |
| Task Process membership | 175,320 / 7,641,629 | 15,007 / 274,770 |
| Work-selection Session summary | 4,401 / 270,064 | 4,401 / 270,064 |

`store/sqlite/processes.rs::agent_processes` changes from the Process primary-
key scan to `agent_process_unfinished`; VM work falls **99.7%**. The warm
profile reads 59 unfinished agents after recovery, rather than scanning the
160,314 retained Processes. Some ambient agents exited since preparation;
profile outputs still match the timing cohort in full.

`store/sqlite/task_work.rs::lf_process_ids` uses `processes_cwd` equality and
range searches; VM work falls **96.4%**. The remaining scans include Session
membership and relocation rows. Its history and attachment lookups remain.
The corrected Work-selection plans use the previously kept Task indexes on
both sides and perform identical work.

Wave inventory SQL takes **187.6 → 0.3 ms** in a **461.5 → 303.7 ms** traced
path. That statement is 40.7% of the traced baseline: deleting it permits at
most a 40.7% latency reduction (about 69% faster), and the observed traced path
reduction is 34.2%. Task inventory plus membership takes **567.6 → 6.3 ms**,
7.1% of its 7960.3 ms traced baseline; its traced path falls to 7829.5 ms.
These are limits on those instrumented runs. The uninstrumented reductions,
194.9 and 508.3 ms, are consistent with hundreds of milliseconds removed from
SQL, within different runs’ variation. Tracing adds work; its path shares
cannot predict the uninstrumented medians exactly. Nested SQL times are not
summed, and no profile timing enters the five-pair result.

### Task status’s remaining non-SQL time

Warm native `sample` runs, separate from timing, return the same full JSON:
**8601.2 ms before; 7156.4 ms after**. Sampling at 5 ms gives these inclusive
main-thread counts (nested rows must not be added):

| Stack | Before, of 1267 | After, of 1105 |
|---|---:|---:|
| Claude Keychain subprocess reads | 752 (59.4%) | 686 (62.1%) |
| Planning transport/config loading | 213 (16.8%) | 182 (16.5%) |
| Whole planning scope check, containing config loading | 275 (21.7%) | 250 (22.6%) |

The dominant stack is `lf/commands/waves.rs::task_detail` →
`ops/human_session.rs::listed` → `provider_account.rs::LaunchCapacity::read` →
`provider_account/identity.rs::same_login` →
`provider_auth/mod.rs::read_claude_keychain_blob_with_security` → child output
and `poll`. `task_detail` emits 84 Session rows and `listed` reads launch
capacity separately for each. Descendant observations also caught repeated
`security` subprocesses; no credential output was collected.

The next stack is `ops/linear_observe.rs::check_scope` →
`ops/planning_client.rs::repository_scope` → `config::load_planning_transport`
→ `config/settings.rs::resolve_layers`, including YAML parsing and validation.
The scope check resolves repository configuration inside its Task loop before
checking freshness. The profiles locate repeated local work, not a long
SQLite lock wait. The initial exploratory sample overlapped compilation and
is retained but excluded from these percentages.

Next measurement target: read launch capacity once for the whole Task detail,
then measure repeated configuration resolution in planning refresh. The
private provider copies omit credential files, so native-login fallback costs
may differ from ordinary homes. The samples identify this reproduction’s
limiter, not a promise that online Task status has the same shares. No further
optimization was made. The original 18–75 s Session-list stall remains open.

### Decision and receipts

Keep both Process changes in `761c28056`: Wave list clears the noise, and the
membership rewrite contributes a measured Task-status improvement without
changing output. No measurable Session-list or Work-selection benefit is
claimed. Source and tests were not modified during measurement.

- Both uninstrumented release builds and both attribution builds passed.
- `cargo fmt --check`; `git diff --check`: passed. Prior focused implementation
  test and clippy results remain above; no Rust edit requires repeating them.
- `scratch/loo-822.md` matches its pre-run hash. The release executable is the
  uninstrumented candidate. No private server or sampler remains running.
- [Every trial](measure.html#iteration2-trials),
  [full samples, hashes and excluded cohort](session-list-measure/iteration2-results.json),
  [bound plans and counters](session-list-measure/iteration2-profiles.json),
  [validation and stack counts](session-list-measure/iteration2-checks.json).
  Raw JSON, stores, stack captures and logs remain in the mode-0700 private
  directory; `unsealed/` holds the invalid cohort unchanged.

## Iteration 3 — Task-status repeated reads (2026-11-11)

Read LaunchCapacity once per Task detail and reuse it for every Session row.
Resolve planning scope once per distinct repository in a scope check, sharing
answers between its Wave and Task loops. Keep all JSON fields, selection,
ordering, freshness, and error handling. No credential values in diagnostics.

Delete — do not maintain: per-row capacity acquisition in `human_session::listed`
and repeated repository scope resolution in `linear_observe::check_scope`.
Keep the existing renderers and scope resolver; no persistent cache or migration.

Focused coverage: Task detail with multiple Sessions and one capacity read;
planning scope checks with multiple Tasks/Waves, shared and foreign teams and
unreadable configuration. Direct callers: Session card and Task screens.
Gate owns wider Session/Task and planning suites; measure owns frozen full-JSON
comparisons and attribution before retaining a performance claim.

- Capacity: `cargo test -p loopflow --lib task_detail_reads_capacity_once_for_all_sessions` passed (1); `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` passed. LF variables unset, stdin closed; existing oversized unwind-table linker warning only. Initial test-counter compile failure used an unstable Cell API, replaced before this pass.

- Scope: `cargo test -p loopflow --lib linear_scope_reads_each_repository_once_and_refreshes_next_check` passed (1), covering one resolution per repository, failed resolutions, live/finished selection and next-check config edits. Fixture borrow and column-name errors were corrected before this pass.
- `cargo test -p loopflow --lib linear_on_demand_adopts_new_open_issues_without_allocating_execution` passed (1).
- Scope fix: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `git diff --check` passed; LF variables unset and stdin closed. Existing oversized unwind-table linker warning only.

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

Reuse iteration 2’s checkpointed `after.db`, with no new live-store backup.
All three standalone files have the same SHA-256, no WAL, and matching digests
for all 78 application tables. Every benchmark sets `LF_HOME` under
`/tmp/loo822-reads-measure`; none runs against the live `~/.lf` store.
The five sealed harness hashes still match. Only side selection and the
requested four workloads change in the iteration-3 driver.

Exact commit exports use the same `cargo build --release -p loopflow --bin lf`,
development provenance, Rust 1.98.1 and macOS 26.0.1 arm64. All builds finish
before their measurements. The clock shim, private transcript copies and
excluded warm-up procedure are unchanged. Calibration distinguishes one row
at 265.8–269.3 ms from the default 100 rows at 435.5–448.1 ms.

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

- Three uninstrumented and two attribution release builds: passed.
- `cargo fmt --check`; `git diff --check`: passed. Production source is
  unchanged, so the focused tests and clippy recorded above are not repeated.
- All frozen store and harness hashes: unchanged. `scratch/loo-822.md` matches
  its starting hash, including the pre-existing edit. No server was started;
  every sampler and command exited. The kept uninstrumented binary is restored
  at `target/release/lf`.
- [Every trial and warm-up](measure.html#iteration3-trials),
  [timings, spawn counts and hashes](session-list-measure/iteration3-results.json),
  [stack attribution and checks](session-list-measure/iteration3-checks.json),
  [leading SQL and counters](session-list-measure/iteration3-profiles.json).
  All 151 calls and their excluded warm-ups are retained; raw JSON, stores and
  captures stay in the private `/tmp/loo822-reads-measure` directory.
