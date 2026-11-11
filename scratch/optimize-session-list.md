# Session-list optimization measurements — LOO-822

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

## Checks and decision

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

The following is the original run, not a comparison to today’s candidates.

## Claim tested and setup

Claim: “On a private copy of the real Machine store, branch-built `lf session
list` completes in the measured median/range, including while a real provider
turn writes rows; its executed query plans identify scans and writer waits.”
The baseline and scan inventory hold; the long-wait explanation remains open.

- Commit `00851d9c6de7ae2beb8779674e2a482fde19269d`, release build:
  `cargo build --release -p loopflow --bin lf`. No optimization was made.
- 2026-11-11, macOS 26.0.1, arm64, bundled SQLite **3.53.2**. One binary for
  both sides, SHA-256 in [results.json](session-list-measure/results.json).
- SQLite online backup from a read-only connection to the real store;
  benchmark commands use `/tmp/loo822-measure/home` or `control`, never the
  live Machine store. Private artifact directory mode 0700. No account
  configuration or token values were printed or copied into the report.
- Snapshot: **14,700 agent_sessions; 235,743 session_events; 12,924
  session_activity; 160,098 processes; 1,096 tasks; 6 provider_accounts**.
  The source main DB file was about 643 MiB; backup includes its WAL.
- Command: `/tmp/loo822-measure/lf session list --json`, in this checkout.
  Inherited `LF_*` variables removed, private `LF_HOME` and `LF_BIN` set.
  Provider homes and current OS processes are real. Warm filesystem caches;
  no cache flushing or synthetic CPU/write load. Unrelated builds were
  visible on this shared Machine during the experiment; it was not quiescent.
- The CLI's default limit is **100**. The server's unpaged `session.list`
  subscription returned **371** rows. Their times are separate baselines,
  not an equivalent-work speed comparison.

## Wall measurements

Timer starts before CLI launch and ends after stdout/stderr are completely
collected. Served timers start when a checkpoint is sent and end when its
answer body arrives. Parsing/checking happens afterward. All timed binaries
are uninstrumented; SQL tracing and EXPLAIN run separately.

| Workload | n | Median | Min–max | Rows / turns | Errors |
|---|---:|---:|---:|---:|---:|
| Default bare CLI, quiet snapshot | 5 | 0.788 s | 0.701–0.863 s | 100 / 11 | 0 |
| Easier CLI, same store, `--limit 1` | 5 | 0.293 s | 0.247–0.329 s | 1 / 0 | 0 |
| Served subscription, warm checkpoints | 5 | 0.963 s | 0.854–1.116 s | 371 / 52 | 0 |
| Writer experiment: quiet copy | 5 | 1.145 s | 1.008–3.088 s | 100 / 11 | 0 |
| Writer experiment: live Pi copy | 5 | 1.742 s | 0.832–2.778 s | 100 / 11 | 0 |

Default and easier calls alternate, reversing order each pair. The harness
clearly distinguishes their workloads; this is not a speed improvement claim,
since they return different work. The server's first reading, excluded from
the five warm readings, took **1.575 s**. Initial bare warm-up took **3.495 s**,
also excluded. No call approached 18 s.

Outputs were JSON-decoded, checked for unique IDs, and each recorded Session's
title/cwd checked against the copy. Turn state and nonnegative usage were
checked. All five full-output digests match within each CLI cohort; all five
warm served digests match. The two homes differ only in `open_argv` paths;
identities, titles, cwd, raw flags and all remaining fields match across the
paired outputs. The core identity digest is
`92c927135d8f037aeefed40f025ddac2a27ffbf567f5f46bac26d57c68106096`.
This validates returned rows, not discovery completeness for every ambient
provider or the correctness of every token count.

### What “concurrent writer” established

A real `lf -b -a pi : …` ran in a private scratch repository and read the
Session-list source to explain its query/locking behavior. Its actual Pi
transcript contained assistant tool calls and tool results. No synthetic
writer, deliberate lock hold, sleep tool or repeated-write workload was used.
The provider run's store added captured/started/observed launch rows.

The first five-pair experiment counted only `session_events`; all five pairs
saw no new events, so it could not show ongoing writes. It is retained as
`control`/`live` in the receipts, not used as the writer verdict above.
A second, separately started baseline kept a read-only SQLite connection open
and compared `PRAGMA data_version` plus `store_revisions` before/after each
CLI call. Every writer-store timing window saw `data_version` change;
process revisions also advanced in four of its five windows. A data-version
change means at least one external commit, **not a count of commits**.
Session/usage revisions and Session-event counts did not move.

The parent runner timed out while Pi continued in its detached transport.
The provider and its two transport processes were then explicitly stopped;
all exited. The provider did not complete its answer. Thus list errors are
zero, but the provider workload has **one incomplete run**. Event-stream
contention is missing, not zero. All private servers/watchers were stopped.

## Executed query inventory

[Every SQL shape and its EXPLAIN QUERY PLAN](session-list-measure/query-plans.md)
is retained, including all schema PRAGMAs, control statements and each main
query's correlated subqueries. [Machine-readable inventory](session-list-measure/queries.json)
includes actual execution counts, VM steps, fullscan steps, sorts and inclusive
profile milliseconds. No parameters or result values are recorded.

A bare trace executed **334 statements, 112 exact SQL strings**. A server
trace of first reading plus two checkpoints executed **2,333 statements,
119 exact strings**. Timestamp normalization combines these into **116
executed shapes**. A one-shot server request added no new shapes (846
statements, 111 exact strings). Two conditional OpenCode reads and one legacy
isolated-account lookup have supplemental plans, for **119 entries**. Plans were generated on the actual traced connections
with NULL placeholder bindings; a parameter-sensitive replan could differ.
The three supplemental plans use the same 3.53.2 amalgamation against the
private store copies, not a timed listing.

| Stage / source under `rust/loopflow/src/` | Query entries | Plan and scale |
|---|---|---|
| Store/repository startup: `store/sqlite.rs`, `store/migrations.rs`, `store/sqlite/durable.rs` | Q001–Q096 | Metadata/migration-ledger and schema scans; 78 `foreign_key_list` PRAGMAs; indexed Machine/repository reads. Private opens also scan `provider_tokens WHERE encrypted=0`; no token rows were emitted by tracing. |
| Launch capacity: `provider_account.rs::LaunchCapacity::read` | Q097–Q098 | Full account/limit index scans, small tables. Native-login detection also reads provider state outside SQLite. |
| Main list: `store/sqlite/sessions.rs::session_summaries_in`, `summary_query` | Q102 | `session_inventory(interactive,completed_at)` selects the page; capture/process/graph/activity joins use PK/index searches. **Repeated Task scans**, wave-address recursive scans/materialization, and JSON virtual-table scans remain. |
| Per-row transition and Task labels: `read_summary`, `store/sqlite/provider_transitions.rs::latest_in` | Q100–Q101 | Transition uses `session_events_history(session_id)` but evaluates JSON while walking that Session's history backward; not a whole event-table scan. Task label uses PK. 105 transition executions bare; 1,128 over three served readings (376 each). |
| Task-ID display and checkout association: `store/sqlite.rs::task_checkouts`, `ops/human_session/workspace.rs` | Q105–Q106, Q005–Q006 | Two whole Tasks scans; checkout resolution then does filesystem/Git work. |
| Live turns: `ops/human_session/turns.rs::current`, `transcript::locate` | Q107–Q109, Q115, Q119 | Session PK, provider-session-account composite PK, activity PK, and `session_events_turn` for completion. No whole event/activity scan here. A legacy isolated account additionally uses `get_provider_account`'s `(provider,account_id)` PK lookup; no such account was reached in these listings. |
| Raw pairing: `turns.rs::raw`, `sessions_of_provider_threads` | Q110–Q112 | **Full `agent_sessions` scan and temp ORDER BY for each candidate**. Five candidates in this run: 73,500 fullscan steps in the bare trace. Five matched Sessions each get a full by-ID summary. This happens before repository filtering; no raw row ultimately belonged to this scoped output. |
| Served revisions / availability: `store/sqlite/revisions.rs`, `sessions.rs::observe_availability` | Q113–Q114 | Tiny revision-table scan. **Unindexed `session_activity` traversal** for MIN: EQP calls it `SEARCH session_activity` with no index, but the runtime fullscan counter proves the traversal. 18 executions / 232,632 steps over three readings: once for the list plus five raw-candidate summaries per reading. |
| OpenCode: `transcript/opencode.rs` | Q110, Q117–Q118 | Candidate search **scans `session_v2` and sorts** (333 rows in this provider-store copy). Messages use `session_message_session_seq_idx`; MAX updated uses `session_message_session_time_created_id_idx` to search by Session, then walks that Session's messages. Copy contains 8,054 messages. |
| Transactions and PRAGMAs | Q001–Q002, Q008–Q016, Q099, Q103–Q104, Q116 | Detailed below. PRAGMA/control EXPLAINs have no relational plan. |

The dominant scans inside Q102/Q112 deserve priority over the raw lookup:

- `task_work::session_tasks("s")` scans `tasks tw` using its primary-key
  index **for every Session**, testing the bound Task or cwd ancestry.
  An index scan over all keys is still O(Tasks × Sessions).
- `EXISTS(SELECT 1 FROM tasks p WHERE p.primary_session_id=s.id)` is a
  correlated **table scan**, again per Session, with early exit on a match.
- `wave_addresses` materializes its recursive address view. `SCAN s` is the
  already materialized page, not another whole `agent_sessions` scan;
  `SCAN r VIRTUAL TABLE` is a Session's JSON status records.
- `session_events` membership/history/capture reads are indexed. This run
  does not show a whole `session_events` scan despite its 235k rows. Indexed
  per-Session JSON filtering can still be expensive for a very long Session.

### Profiling and limits on speed claims

In the separate bare profile, the main Q102 took **256 ms inclusive**,
**1,994,515 VM steps**, **196,487 fullscan steps**. The five by-ID raw summaries
took 10 ms; raw thread matching took 8 ms; schema fingerprinting took 20 ms.
The three served main queries took **901, 889, 844 ms**, each with **760,251
fullscan steps**. Served availability scans totaled 27 ms and raw thread
matching 25 ms across the three readings. This maps the SQL hot spot to
`summary_query` and its Task-association subqueries, rather than to a write
lock or whole event-table scan.

PROFILE includes nested SQL and the tracing callback's time; summing all
statement times double-counts nested work. SQLite reports milliseconds, so
zero is below resolution. These are attribution runs, excluded from wall
medians. Two macOS `sample` attempts returned no stack samples before process
exit; CPU and non-SQL wall attribution are missing.

For scale only: raw matching's 8 ms is **0.54%** of the 1,480 ms traced bare
call. Deleting all of it could save at most that share of **that traced
call** (about 0.54% faster), not explain a 75 s stall. The 256 ms main query
is 17.3% of that call; removing it entirely caps that traced-call speedup at
about **21%**. The callbacks/first traced launch distort the denominator;
these are **not forecasts for the uninstrumented CLI**. The repeated Task
work has the largest measured SQL cost; its actual wall benefit requires a
fresh, frozen before/after experiment.

## Where a list can wait

**Normal measured path:** no SQL data writes, no `BEGIN IMMEDIATE`, no
`BEGIN EXCLUSIVE`, no lock failure or retry observed. Bare trace: four
`BEGIN DEFERRED`/COMMIT pairs and seven SAVEPOINT/ROLLBACK TO/RELEASE groups.
Three served readings: seven deferred begins, four commits, three outer
rollbacks, 21 savepoint groups. The extra savepoint per list is checkout
reading; five more are raw-candidate summaries. Savepoints do not reserve
the writer lock. An unfinished write in ordinary WAL mode does not block
these ordinary reads; this is not a claim about WAL recovery or schema work.

Potential waits recoverable in code: the bare opener takes `.migration.lock`
and can migrate; served subscription reads pin a read-only snapshot;
observation-cache misses serialize per fact. None identifies the stall without
a captured wait. The trace reached neither pending migrations nor plaintext
provider-token conversion. One-shot server requests still took the bare-open
path inside an observation capture, unlike subscriptions.

Transcript reads were filesystem work except OpenCode. Its 200 ms busy timeout,
read-only opens and 333-row candidate scan were observed on this provider copy;
the provider DB was never modified. Raw discovery inspected all Machine
processes before repository filtering. The runtime path can be reconstructed
from `transcript::locate`, `transcript::raw` and `observe`; no stack sample
caught which non-SQL span dominated a slow listing.

## Scope and next measurement

This inventory covers default, repository-scoped CLI listing and the unpaged
served subscription, including their startup and conditional transcript
reads. Filter-specific history/search/task/account pagination queries are
not executed here and should not be described as benchmarked. The inherited
real provider set produced five raw candidates and zero displayed raw rows;
a directory with many unnamed raw conversations can amplify Q111/Q112.

The next optimization experiment should target repeated Task association and
primary-Session scans in Q102, retaining all behavior and comparing the same
100/371-row outputs. Raw-pairing lookup and repeated served availability
scans are smaller independent candidates. No index, resolver or cache change
is included in this baseline.

To settle the stall: capture stacks **while** a slow CLI/server reading is
still alive, plus SQLite statement begin/end and transaction state and the
holder of `.migration.lock`; record external read/cache-entry spans too.
Repeat the writer windows with a provider turn that visibly commits
`session_events` throughout them. A missing slow run cannot identify its
lock holder from code alone.

## Reproduction and retained artifacts

`session-list-measure/run.py` is the fixed CLI timer/validator;
`served.py` drives the real private server; `live.py` records the first
provider attempt; `writer_windows.py` is the restarted commit-aware baseline.
Results retain individual wall samples, errors, row counts and digests.
`trace.rs` is the temporary PROFILE/EXPLAIN callback, installed via
`performance::observe_sqlite` plus OpenCode's `open` for the attribution
build only. Production source was restored byte-for-byte afterward. Trace
SQL captures no bound values. Raw stdout, store copies and provider logs
stay in the mode-0700 `/tmp/loo822-measure` directory.

Checks: release build passed; all 25 reported CLI/subscription measurements
returned valid output; two additional excluded warm-ups and the initial
10 paired CLI measurements are retained as described above. No Rust production
code changed, so no additional unit suite was run. Provider execution was
incomplete and cleaned up, not counted as a successful provider run.

## Implementation checks (2026-11-11)

- `cargo test -p loopflow --lib -- <focused filters>`: 13 distinct tests passed across task_work, sessions, session_events, selections, children, waves, Steers and attachment retirement; includes migration plans/VM bound and snapshot composition. LF variables unset, stdin closed. Initial fixture errors and the ordinary JOIN's checkout scan were fixed before these passes.
- `uv run python scripts/check_migrations.py`: passed against origin/main; one additive migration.
- `cargo fmt`; `cargo clippy --all-targets -- -D warnings`: passed.
- Gate: remaining affected suites, including `two_people`, `transport_tests`, and end-to-end board/Session acceptance. Measure: frozen full-JSON comparisons and alternating wall times above.
