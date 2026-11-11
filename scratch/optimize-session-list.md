# `lf session list` baseline — LOO-822

**Inconclusive for the reported 18–75 s stall.** It did not recur. A warm
release-built default list took **0.788 s median, 0.701–0.863 s, n=5**, with
100 correct rows and no failures. The main Session summary query dominates
measured SQL work. There are real scans to reduce, but no measured long
SQLite writer wait to attribute the old stalls to.

The live-provider comparison showed **no measurable difference** under the
skill's overlapping-range rule: quiet-copy median **1.145 s** (1.008–3.088),
live-Pi-copy median **1.742 s** (0.832–2.778), five alternating pairs, zero
list errors. Every timed window on the writer's store observed a commit.
Those were process-bookkeeping writes: no new `session_events` rows arrived
in these windows. This does **not** establish performance under a sustained
Session-event write burst.

Jack Heart: “Lets take a look at all the queries that happen during lf
session list and look for places we get stuck behind and a write lock or do
a table scan”. The 75 s and 18 s observations are in `qa.html`; they are not
measurements from this run.

## Scope (2026-11-11)

Jack Heart: "Lets take a look at all the queries that happen during lf session list and look for places we get stuck behind and a write lock or do a table scan"

Jack Heart, next message: "you can also do a cursory scan of other common planning queries and optimzie those as well"

## Next experiment — iterate (2026-11-11)

Implementation built in two commits; wall-time measurement remains. Compare
baseline `00851d9c6` to Task-probe commit `d9e3c1f3b`, then that commit to
the following smaller query-fixes commit. Apply each commit's migration to a separate frozen copy;
the final migration edits the same file, so do not reuse a copy already
migrated by the first experimental build. No claimed speedup yet.

**Hypothesis:** replace the two per-Session Task scans in Q102 with indexed
lookups: binding by Task ID, checkout membership by the Session cwd's ancestor
paths, and primary ownership by Session ID. Work should scale with path depth
and actual matches instead of all Tasks for every returned Session. Aim for
about **15% off default CLI median** (0.788 → 0.670 s) and **25% off served
median** (0.963 → 0.722 s), with identical outputs. These are experiment
targets, not measured improvements or estimates derived from PROFILE time.
No overall latency target was supplied; a worthwhile next experiment remains.

Read the sole baseline row in `measure.html`: no optimization has been rejected
yet. The live-writer comparison was inconclusive, not a rejected query change.
Do the main change first and measure it alone; then measure the smaller fixes
below independently so their benefit is not credited to Q102.

### Delete — do not maintain (removed)

- The full-Task reverse scan in `task_work::session_tasks` and the duplicate
  predicate in `session_task_ids`; keep the forward membership rule unchanged.
- Q114's arithmetic WHERE predicate; keep its deadline registration loop.
- Repository retain-after-decoding in `selections::work_in` and
  `waves::roadmap_snapshot`; keep global identifier inventory and Task mapping.

### 1. Replace reverse Task scans with indexed probes

The indexed `store/sqlite/task_work.rs::session_tasks` is shared by summaries,
orphan filtering, Steer membership, Session retirement and `session_task_ids`.
The forward `session_ids`/`session_membership` rule remains unchanged.

- Split binding and checkout matches with `UNION`, not `UNION ALL`. Probe
  `tasks.id` for the binding. Enumerate lexical ancestors of cwd once per
  Session, then probe local checkout roots. Sort the resulting IDs exactly as
  today, including when binding and cwd name different Tasks.
- Add a partial expression index on
  `tasks(checkout_machine_id, rtrim(worktree,'/')) WHERE worktree!=''`.
  The existing unique `tasks(worktree)` index cannot serve the current
  `instr(cwd,rtrim(worktree,'/')||'/')` predicate.
- Add `tasks(primary_session_id) WHERE primary_session_id IS NOT NULL`.
  Keep Q102's `EXISTS` expression; it becomes an indexed existence lookup.
- Keep the materialized Session page, filtering before pagination, row order,
  JSON ID order, primary-scope exclusion and existing read snapshot. No stored
  membership cache, new resolver, write-on-read or revision domain is needed.

The existing `task_graph_pairs` ancestor expression is a useful starting point:
`rtrim(rtrim(path, replace(path,'/','')),'/')`. Seed with `rtrim(cwd,'/')`.
Include the empty root only for an absolute cwd or an empty cwd: today a Task
worktree `/` trims to `''` and matches absolute paths, but not a nonempty relative
path. Do not canonicalize paths or replace containment with SQL `LIKE`.
Test nested roots, trailing/repeated slashes, relative/empty cwd, root checkout,
`%`/`_`, sibling prefixes, NULL/empty worktrees, foreign Machine checkouts,
explicit binding, duplicate binding/checkout matches and primary scopes.

A feasibility probe on 2026-11-11 copied only the needed columns from the
private store into in-memory tables. On Python SQLite **3.50.4**, all **14,701**
Sessions' ordered membership arrays and primary flags matched the old query.
The single full-inventory execution was 5,311.1 ms before versus 91.3 ms after;
EQP showed Task PK, checkout-expression and primary-Session index searches.
This is a reduced SQL probe, not a CLI benchmark: it omits the rest of Q102,
uses a different SQLite from lf, and the private store has one more Session
than the original 14,700-row snapshot. Recheck with bundled 3.53.2 on a frozen
copy. A small temporary sort of matched IDs is expected; a scan of every Task
per Session is not.

### 2. Smaller fixes worth trying separately

| Change | Mechanism and limit |
|---|---|
| Add `agent_sessions(provider_thread, created_at DESC)` | Q111 can seek each raw candidate and stop at the newest row instead of scanning 14,700 rows and sorting. Keep current matching semantics; use the two-column nonunique index and test equal timestamps so an added covering ID does not change tie selection. Baseline attributed only 8 ms bare to this work. |
| Add `session_activity(observed_at)` and rewrite Q114 | Use `SELECT MIN(observed_at)+?1 FROM session_activity WHERE observed_at>?2-?1` so the deadline read can seek the next timestamp. Keep the deadline loop and observation registration, including the exact quiet-time boundary and empty-table NULL. An index alone does not fix the arithmetic predicate. Baseline attributed 27 ms across three served readings. |
| Push known repository scope into Task SQL | In `selections.rs::work_in` and `waves.rs::roadmap_snapshot`, avoid decoding all repositories' Tasks and then retaining one repository in Rust. Extend the existing Task reader with a bound repository predicate using `idx_tasks_repository`; reuse its visibility, open/history and row mapping. Keep global reads global and exact Task selection's repository semantics. |

All four indexes live in this Task's one additive migration,
`session_list_indexes.sql`, created with `new_migration.py`. The upgrade test
starts at the merged frontier. Provider databases remain unchanged.

### Planning-read scope

Keep forward `session_ids` unchanged: supplemental Python SQLite 3.50.4 EQP
showed its Session scan, but no measured payoff yet. Wave counts already used
`idx_projects_wave_updated`, `idx_tasks_project` and the deletion PK. Leave
Wave-address materialization and per-Task detail reads for later measurement.
`TaskIdentifiers::read` must keep all repository IDs so short prefixes remain
globally unambiguous. Repository predicates used `idx_tasks_repository` in the
supplemental EQP; NULL bindings there require a real-bound bundled-SQLite check.

### Verification and decision

1. Keep the existing release binary and original receipts. Freeze a fresh
   private store and external observations for an alternating before/after
   comparison; do not silently compare today's candidate to the old wall times
   or the writer experiment's mutated store. Use identical paths for both
   binaries so `open_argv` stays comparable. Record binary hashes and store
   counts. No provider calls are needed for this query experiment.
2. Run five alternating pairs per workload, reversing order each pair: default
   100-row CLI and unpaged 371-row served checkpoints separately. Exclude and
   report warm-ups as before. Require matching full JSON digests and zero
   errors for each frozen workload, plus the existing identity/content checks.
   If ambient processes, transcripts or deadlines change an output, isolate
   those inputs and rerun; do not hide the difference by dropping fields.
3. Trace separately with bundled SQLite. Require indexed Task probes in
   Q102/Q112 and a substantial drop from 196k/760k fullscan steps; inspect VM
   steps too so recursive work has not merely hidden the scans. Check Q111
   loses its full scan/sort and Q114 becomes a timestamp range search when
   their separate changes are tried. Never use trace times as CLI wall times.
4. Exercise `--task`, `--orphan`, history, pagination and by-ID summaries;
   compare Task status, Wave status/list, repository/all-repository boards
   and desktop selections against unchanged fixtures. For scoped Task reads,
   include duplicate issue identifiers across repositories, deleted/closed
   Tasks, unassigned Tasks and short-ID collisions. Inspect filtered plans
   with real bindings; keep output and TaskSection decisions identical.
5. Implement coverage: focused `task_work`, `sessions`, `session_events`,
   `selections`, `children` and relevant `waves`/`screens` tests; membership
   changes also reach `steers` and Process retirement. Run migration checks
   and snapshot-composition tests. Before committing run `cargo fmt` and
   `cargo clippy --all-targets -- -D warnings`; unset inherited `LF_*` for
   Cargo tests and close stdin. Gate runs the remaining affected suites.

Retain an optimization only with unchanged results and a non-overlapping
before/after timing range for its claimed wall benefit. Overlap is
inconclusive; use the trace to choose the next experiment, not to invent a
speedup. Record every trial in `measure.html`, including rejected trials.
If the main change reaches the experiment targets, remeasure the limiter
before continuing. Advance when the remaining ideas no longer justify their
cost; blocked only if the workload or harness prevents trustworthy results.

### Ruled out for this iteration

Keep membership fresh and the page size unchanged: no cache, new resolver,
write-on-read or revision domain. Existing unique `tasks(worktree)` cannot
seek the reverse `instr` predicate. Provider databases stay read-only; no
OpenCode index or discovery cache. Do not change locks/timeouts to explain the
unreproduced stall. If indexed probes do not move wall time, measure non-SQL
spans on the frozen workload before another rewrite.

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
