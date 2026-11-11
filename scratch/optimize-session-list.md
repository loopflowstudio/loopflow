# Session-list optimization measurements — LOO-822

**Iteration 2: keep `761c28056`.** On sealed private copies, Wave-list median
falls **44.2%** and network-denied Task-status median **6.9%**, with matching
full JSON. The first, unsealed cohort is invalid; its apparent Work-selection
gain is withdrawn. Full results and the remaining limiter are below.

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

## Fresh comparisons (2026-11-11)

Claim tested: “On the frozen real Machine store, indexed reverse Task probes
reduce default CLI median by at least 15% and warm served median by at least
25%, without changing the answer.” The smaller fixes must earn their own gain.

`before` = `00851d9c6`; `after1` = `d9e3c1f3b`; `after2` = `f8b742d8c`.
Each table entry is **median [minimum–maximum], milliseconds; n=5 per side**.
Pairs reverse order on every iteration. Each timed CLI follows an excluded
warm-up; each server is warmed with an initial subscription before timing its
checkpoint. SQL tracing is excluded from these times.

| Comparison | Workload | Earlier | Candidate | Verdict |
|---|---|---:|---:|---|
| before → after1 | cli | 579.0 [561.7–586.6] | 436.3 [421.9–443.0] | Better |
| before → after1 | served | 775.3 [747.3–885.5] | 210.7 [196.2–217.2] | Better |
| after1 → after2 | cli | 439.7 [420.7–461.3] | 420.2 [410.5–429.5] | No measurable difference |
| after1 → after2 | served | 204.4 [190.2–209.6] | 198.1 [179.5–234.9] | No measurable difference |
| before → after2 | cli | 572.3 [565.4–584.6] | 428.0 [418.7–440.5] | Better |
| before → after2 | served | 785.2 [758.4–970.9] | 201.1 [190.5–556.0] | Better |
| after1 → after2 | board | 856.1 [825.0–1263.0] | 853.7 [771.9–1344.6] | No measurable difference |
| after1 → after2 | selection | 458.5 [437.9–487.0] | 482.4 [461.3–905.9] | No measurable difference |
| before → after2, network denied | task | 7687.8 [7033.9–8213.9] | 7301.6 [6953.0–7791.0] | No measurable difference |
| before → after2, network denied | waves | 357.6 [351.9–394.0] | 385.5 [342.1–700.5] | No measurable difference |
| before → after2, network denied | board | 1388.9 [959.3–1513.7] | 1391.9 [1150.4–2555.0] | No measurable difference |
| before → after2, network denied | selection | 1627.5 [1235.4–2640.2] | 501.0 [467.5–650.6] | Better |

The cumulative Work-selection improvement does not establish a benefit from
`f8b742d8c`: the separate after1/after2 comparison overlaps. That reading also
uses the Session summary query improved by `d9e3c1f3b`. Task status and Wave
list show no measurable difference. Network-denied results are local-query
measurements, not end-to-end planning-exchange latency.

## Setup and validation

- Fresh read-only SQLite online backup of the Machine store, then independent
  copies for each migration. All application tables match the frozen backup;
  only `schema_migrations` differs. Frozen counts: **14,731 Sessions, 236,270
  Session events, 12,955 activity rows, 160,214 Processes, 1,099 Tasks and six
  provider accounts**. The live source gained two events between backup and
  the initial count query; the report uses the backup’s counts.
- The original release baseline binary is retained unchanged. Both candidates
  were release-built with `cargo build --release -p loopflow --bin lf`, using
  the corresponding Rust source blobs in this checkout. Build provenance for
  reconstructed after1 still names the checkout’s HEAD; source blobs, binary
  hashes and commit mapping are recorded separately. Same Rust 1.98.1, Cargo
  1.98.1, macOS 26.0.1 arm64, 16 logical CPUs and 128 GiB memory.
- All commands run in this checkout with inherited `LF_*` removed and private
  `LF_HOME`/`LF_BIN`. Binaries take turns at the same executable path; independent
  store copies take turns at the same home path. `open_argv` therefore needs no
  normalization. Clone/reset and validation happen outside timing.
- Claude/Codex transcript directories, Claude owner records, Pi’s Session
  directory and an online backup of OpenCode’s database are private copies.
  No provider-home credential files were copied. A private clock interposer fixes wall time at
  Unix **1794415563**, leaving monotonic clocks untouched. Timestamp fields and
  deadline decisions remain in the full JSON digest.
- OS processes, filesystem/Git facts and legacy absolute transcript paths
  remain ambient. These are limits, not claimed frozen inputs. Full-answer
  digests match within every retained cohort. Shared-Machine one-minute load ranged from 12.7 to 222.5;
  no other person’s processes were stopped and no test load was generated.
- **120 retained timed calls and 120 excluded warm-ups**, with zero command
  or validation failures. CLI returns 100 unique Sessions and 11 turns;
  served returns 371 unique Sessions and 52 turns. Task/Wave/board/selection
  counts are 1/7/183/27. Session titles/cwd and Task titles are checked against
  the private store; turn state and nonnegative usage are checked. This does
  not validate every historical token count or raw-discovery completeness.
- The calibration distinguishes a one-row list (498.7–539.7 ms) from the
  default list (1,040.3–1,900.5 ms), two alternating pairs during setup. It
  is an easier workload, not a performance improvement claim.

## Excluded and incomplete work

- One preflight board validator used the wrong key (`status`, not `state`).
  Fixed before freezing the timing harness; its failed receipt is retained.
- The first Task-status cohort was stopped after one completed baseline trial
  (8,335.8 ms) when the trace showed planning exchange and private-store writes.
  The next candidate trial was interrupted; its time and error count are
  missing, not zero. The replacement planning cohort denies network access
  on both sides. Command success does not mean the blocked exchange succeeded;
  rejected network operations were not counted. Normal online Task-status
  performance remains inconclusive.
- Initial trace collection had 72 interleaved task-trace lines, and SQLite’s
  own elapsed counter read the frozen wall clock. Those traces are excluded
  from final attribution. The corrected collector serializes writes and uses
  monotonic statement start/end times.

## SQLite attribution

Bundled SQLite **3.53.2**, release attribution builds, actual bound EXPLAIN
plans on the traced connection. Statement timers use a monotonic clock;
SQLite’s wall-clock PROFILE duration is unusable under the clock freeze.
Times include row handling, nested reads and tracing overhead. Do not sum
nested times or substitute them for the uninstrumented medians.

Counters below are **fullscan steps / VM steps**. Q102 is per execution;
Q111/Q112 are totals for five raw-candidate lookups in one bare CLI reading;
Q114 is the six deadline reads in one served reading. Both served executions
(initial subscription and warm checkpoint) have the same step counts.

| Query / workload | before | after1 | after2 |
|---|---:|---:|---:|
| Q102, CLI 100 rows | 196,961 / 1,999,578 | 4,129 / 85,913 | 4,129 / 85,913 |
| Q102, served 371 rows | 762,015 / 7,619,268 | 4,400 / 267,265 | 4,400 / 267,265 |
| Q111, five thread lookups | 73,650 / 221,050 | 73,650 / 221,050 | 0 / 70 |
| Q112, five by-ID summaries | 31,440 / 203,904 | 20,460 / 100,404 | 20,460 / 100,404 |
| Q114, six availability deadlines | 77,724 / 311,274 | 77,724 / 311,274 | 0 / 108 |

- Q102/Q112 use Task ID searches, `tasks_checkout_root` expression searches
  and `tasks_primary_session` covering searches. Q102 loses **97.9% of CLI
  scans / 95.7% of VM steps**, and **99.4% of served scans / 96.5% of VM steps**.
  The recursive ancestor work has not merely hidden scans. It adds 100 small
  sorts in the CLI and 371 in the served list; full answers remain identical.
- After2 Q111 uses `session_provider_thread_created`, removing all five sorts
  as well as the Session scan. Q114 uses the covering timestamp range search
  `session_activity_observed (observed_at>?)`; it loses the traversal.
- Real repository bindings use `idx_tasks_repository` for after2’s Task read.
  Board Task-read scans/VM fall from **1,098 / 24,431** to **0 / 22,154**;
  selection Task-read scans/VM from **1,098 / 79,548** to **0 / 73,304**.
  The copy contains 730 Tasks in this repository out of 1,099 globally.
  The global identifier inventory remains global.

The hot path maps to `store/sqlite/sessions.rs::summary_query` and
`store/sqlite/task_work.rs::session_tasks`. In the later warm attribution
run, CLI Q102 falls **186.1 → 21.5 ms**; warm served Q102 **915.5 → 118.3 ms**.
Those reductions explain the direction and scale of the kept change.
Q102 occupies 26.6% of the 701.0 ms traced baseline CLI and 79.8% of the
1,147.7 ms traced warm served path. Even deleting it entirely could reduce
those paths by at most those shares (about 36% / 394% faster); the measured
24.7% / 72.8% latency reductions do not exceed that upper bound. These are
limits on the traced runs, not exact predictions for uninstrumented calls.

After1’s bare Q111 totals **12.8 ms / 666.8 ms = 1.9%** of its traced CLI.
Warm served Q111 plus Q114 total **30.5 ms / 310.0 ms = 9.8%**. The extra
indexes make those statements cheaper, but their wall-time ranges overlap.
Scoped Task reads take only **1.5 ms** in the after1 board trace and **3.6 ms**
in the after1 selection trace, dropping to 1.5 and 2.6 ms. Micro gains at that
share of the path do not justify claiming a planning speedup.

After the Task probes, the main CLI summary is only **3.2%** of its traced
path; the warm served summary remains **38.2%**. Its remaining scans include
wave-address materialization. A separate visible planning limiter is the
live-agent Process inventory: **160,213 fullscan / 673,783 VM steps**, 247.4 ms
of a 475.8 ms Wave-list trace. Task status also spends 569.9 ms on a
membership-based Process inventory (**175,186 scans / 7,635,331 VM steps**).
These map to `store/sqlite/processes.rs::agent_processes` and `task_work`
membership queries;
Task status’s remaining multi-second non-SQL work was not isolated. Those
are candidates for a fresh experiment, not changes made here.

**Attribution limit:** by the time the corrected builds finished, two recorded
CLI AgentProcesses and 15 served AgentProcesses had exited. Kernel probes
confirmed those PIDs no longer existed. Profile answers therefore contain
9/37 turns rather than the timed cohort’s 11/52, although all three profile
builds match each other’s full JSON and still return 100/371 Sessions.
The SQL comparisons use the same frozen store and matching trace workload;
whole-path profile times are not an interchangeable repeat of the timing
cohort. No fields were dropped to conceal this difference.

[All selected SQL, bound plans, per-execution counters and top planning queries](session-list-measure/comparison-profiles.json)
include the three attribution binary hashes. The JSON’s `non_readonly_statements`
is SQLite’s classification, not a count of committed data changes.

## Iteration 1 checks and decision

- Full JSON matches on all three binaries for Task filtering, orphan filtering,
  history, both pages of a three-row pagination check, by-ID cards, all-repository
  board (272 Tasks), empty Wave status and active Wave status (seven Tasks):
  [24 primary answers plus three continuation pages](session-list-measure/comparison-acceptance.json).
- `uv run python scripts/check_migrations.py`: passed; the retained additive
  migration contains only the checkout-root and primary-Session indexes.
- All candidate Rust source files equal `d9e3c1f3b` byte-for-byte after rejection;
  the frozen timing harness hashes are unchanged. Existing fixture coverage
  includes lexical paths, cross-repository identifiers, deleted/closed Tasks,
  scope exclusion and snapshot composition (historical implementation checks below).
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p loopflow --lib store::sqlite::task_work::tests::`: **11 passed**,
  LF variables removed and stdin closed. The test linker emitted an oversized
  `__eh_frame` compact-unwind warning; no test or clippy failure.
- `git diff --check`: passed. All private servers were terminated after each
  read. The release binary is restored to the measured, uninstrumented after1
  build. The original baseline binary is unchanged.

Keep the measured Task probes. Leave the smaller fixes rejected until a real
workload shows a non-overlapping incremental benefit. No new cache, resolver,
lock policy or query rewrite was added during measurement.

## Artifacts

- [Every timing trial and decision](measure.html)
- [Hashes, full samples, warm-ups, counts and excluded trials](session-list-measure/comparison-results.json)
- `session-list-measure/compare.py`, `compare_series.py`, `compare_followup.py`
  and `planning_compare.py`: frozen comparison harnesses; `build_traces.py`,
  `trace_bound.rs`, `profile_compare.py`: separate attribution builds and runs.
- Raw answers, databases, transcript copies and build logs remain in the
  mode-0700 `/tmp/loo822-compare`; the original `/tmp/loo822-measure` is intact.

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
