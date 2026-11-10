# Subscription experiment — 2026-11-09

Step 3 of the existing optimize Graph. Source checkpoints `4d7de3695c` and
`ea614fc0c0715ec91ad3da6fc07fd859055af993` remain. Pre-existing scratch hashes
are in `/tmp/subscription-experiment-20261109/preserved.json`; those files,
including the design and measure.html, remain untouched.

Timing scope is reduced to unpaged `session.list`. This isolates the common
Reader runtime without Task identifier resolution, checkout Git or the separate
ActivityReader store confounding attribution. It supports Session subscription
admission, checkpoint, churn, cleanup and duplicate-projection claims only.
Task and Process subscriptions, distinct Task demand and total installed server
CPU remain unmeasured. Keep 1/16/64 subscriptions, multiplexed and physical
connections, actual dispatch, 32/128 sequential cycles and five cohort cycles.
Fixture runtime has two async workers and four blocking workers; reader runtimes
use `TOKIO_WORKER_THREADS=2`, bounding the 64-reader cohort to about 260 threads
rather than creating a core-count-sized pool for each reader. This is a declared
benchmark configuration, not a production limit change.

Borrowing the server Handle is rejected as an ownership design: outer pulls
occupy server blocking workers while Store::run_sqlite awaits nested work on
that same pool. A two-slot deterministic saturation check must demonstrate the
cycle and sibling progress after release. Candidate alternatives are a separate
executor scoped to server lifetime, or per-reader current-thread runtimes.
The latter retain independent blocking pools, remove idle async worker creation,
and change no snapshot/checkpoint ownership. No candidate is selected before
the frozen baseline identifies material runtime lifecycle cost.

Delete — do not maintain (conditional on measurement): Reader's multi-thread
runtime construction. Preserve its private snapshot/config/checkpoint/error
state and existing attribution. No projection broker, store pooling, altered
clocks, process-policy changes or provider operations.

Decision, 2026-11-09: no production runtime candidate. The frozen baseline and
bounded default pilot do not establish a credible 20% whole-cycle CPU gain from
runtime lifecycle removal. This does not establish an upper bound on runtime
contention under normal concurrency. Parent owns the next measure/design
decision and broader gate; there is no A/B candidate pair yet.

Pilot receipts (excluded from frozen measurements):
`/tmp/subscription-experiment-20261109/pilot1` stalled before timing because
libtest prefixed the writer's readiness line; its log and stack sample remain.
The parser now finds the marker within the line. Initial build logs retain
fixture compile errors (undeclared git2, trace-mask type and u64 SQL decoding).
`pilot2` passed at one subscriber: 27.30 ms server CPU for admission/checkpoint;
128 cycles used 3,323.24 ms, versus 9.20 ms runtime build/drop caller CPU.
Caller CPU excludes runtime worker CPU. Ten writer events produced ten retained
receipts. All 167 runtimes, 502 runtime threads and 167 wake threads released;
FDs returned to 17, threads to 6, with no SQL/projections in 2.2 s after cleanup.
These are calibration observations, not the frozen comparison.

Focused checks: optimized library binary, inherited LF_* unset, stdin closed,
TOKIO_WORKER_THREADS=2: 10 subscription/cancellation/saturation tests passed.
The two-slot borrowed-executor fixture held both outer workers, observed zero
nested SQLite completions, then released and joined every job plus its sibling.
Independent current-thread readers completed SQLite work while both outer
workers were held. The separately invoked ignored queued-cancel acceptance
failed deterministically: one runtime was constructed after cancellation, where
zero was required. Its output is `queued-cancel-baseline.log`; this is existing
production behavior, not a passing acceptance check. No production fix has
been selected.

Frozen source: `e7061670d` (seven source/test/doc files; no runtime policy change).
Executable: `/tmp/subscription-experiment-20261109/frozen-server`, SHA-256
`93bb91ba56a207d14105c3df598c913e249ff3757ba45b687aa1719ac39d3015`.
Runner: `/tmp/subscription-experiment-20261109/frozen-runner.py`, SHA-256
`6ee7f202190956c8f95adeebd491fe6b0eeb7214814fcbb94dee0cd5e2af4dd3`.
The binary was built from the checkpoint's Rust content before the checkpoint
commit; its embedded Git build label therefore predates that commit. The
executable digest, source checkpoint and per-source hashes identify this freeze.

Exact build/check commands (LF_* removed and stdin closed for Rust):

```sh
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo test --release -p loopflow --lib subscription_ --no-run < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') TOKIO_WORKER_THREADS=2 target/release/deps/loopflow-ef39d37f77a3438d server::service::subscription_measurement::borrowed_executor_saturation server::service::subscription_measurement::independent_current_thread server::service::tests::cancellation_during_setup server::service::tests::cancel_ends_a_subscription lf::commands::subscribe::tests:: --test-threads=1 --nocapture < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') TOKIO_WORKER_THREADS=2 target/release/deps/loopflow-ef39d37f77a3438d --exact server::service::tests::queued_subscription_cancellation_skips_construction --ignored --nocapture --test-threads=1 < /dev/null
env $(env | grep '^LF_' | cut -d= -f1 | sed 's/^/-u /') cargo clippy --all-targets -- -D warnings < /dev/null
cargo fmt --check
uv run ruff check scripts/benchmarks/subscription_baseline.py
```

Build, 10 focused tests, fmt, Ruff and clippy passed; the ignored queued-cancel
acceptance failed (1 constructed runtime, required 0). Logs retain each result.

Frozen timing command:

```sh
uv run python /tmp/subscription-experiment-20261109/frozen-runner.py /tmp/subscription-experiment-20261109/frozen-server --output /tmp/subscription-experiment-20261109/baseline --rounds 5
```

Parent agent review, 2026-11-09: “do not reject runtime lifecycle as
immaterial to normal settings solely from that capped workload.” Source:
`.lf/prompts/20261109-154426-b7cf4370-de2a-4f3c-87eb-dd80489d58ef-xSOHWs-steer.md`.
The capped freeze stays unchanged. A separately frozen fixture/runner adds
`--default-config --counts 1,16 --multiplexed-only`, removing both reader and
server Tokio caps; the population bound keeps resource creation finite. This
pilot/profile precedes the implementation decision and is not mixed into the
capped matrix.

Fixture `ps` inventories happen outside every measured phase. Cumulative
before/after-shutdown child CPU includes seeding and those inventory children,
so it is not application child CPU. Phase child-CPU deltas exclude inventories.
The new default pilot additionally reports each inventory child's CPU separately.

The default pilot includes a separate `runtime_only32` phase: 32 sequential
ordinary Tokio runtime constructions and drops, with no subscribers or SQL.
Whole-process CPU includes worker initialization and shutdown through each join;
at most one additional runtime lives. This component probe is separate from
actual connection-path cycles and is not counted as a reader projection. It
can show whether caller-only runtime timings omitted material worker CPU.

Capped matrix: 25/25 fresh-process samples passed. Median complete-cycle server
CPU (admission, first reading, checkpoint, cleanup): 1/1 subscription/socket
25.85 ms; 16/1 1,456.73 ms; 16/16 1,488.45 ms; 64/1 6,025.45 ms;
64/64 5,790.73 ms. Every sample balanced reader runtimes, runtime threads and
wake threads, restored warmed FDs, and recorded zero SQL after cleanup.
Shared server blocking workers grew from two to four and remained idle until
shutdown. Median active-window projections: 10, 160, 160, 306, 306 respectively;
the 64-reader rest/coalescing behavior is visible, not normalized away.
Runtime build/drop caller CPU remains a lower bound, not the default-runtime
decision. All raw data and the unchanged frozen runner are retained.

Default pilot freeze: `17c304389`. Executable
`/tmp/subscription-experiment-20261109/default-server`, SHA-256
`a8531bca7c88678b666ad1d793276f08c14bbb6230b44d8a30916fb6bf071413`;
runner `default-runner.py`, SHA-256
`6234e65e295657778041c807818e4eefea09293a5eba2697eb2459dbcb4e9fc6`.
Build, all-target clippy (44.37 s), fmt and Ruff passed. The only changes since
the capped freeze are the opt-in fixture defaults/probe, writer validation,
inventory CPU accounting and their documentation; production remains unchanged.

```sh
uv run python /tmp/subscription-experiment-20261109/default-runner.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-experiment-20261109/default-pilot --default-config --counts 1,16 --multiplexed-only --rounds 1
```

## Default pilot result and revised ownership proposal

Two unprofiled fresh-process samples completed (one each at 1 and 16 clients),
plus two separate profiled samples excluded from timing comparisons. At ordinary
16-core runtime defaults:

| Subscriptions | Complete-cycle server CPU | Pure runtime CPU per create/drop | Active server CPU / 10 s | Active projections |
| --- | ---: | ---: | ---: | ---: |
| 1 | 27.74 ms | 1.371 ms | 86.43 ms | 10 |
| 16 | 4,980.68 ms | 1.258 ms | 10,430.56 ms | 160 |

The pure component is a sequential probe, not a concurrent-runtime upper bound.
Multiplying it by client count predicts only ~5% of the single-client cycle and
~0.4% of the 16-client cycle; neither supports the 20% whole-cycle target by
itself. Runtime workers may interact with concurrent store opening and locks;
that interaction has not been isolated. The default/capped difference is not an
A/B speedup: both server and reader concurrency differ, and default n=1 per shape.

The 16-client admission/checkpoint phase used 4,874.90 ms server CPU; measured
store-opening caller CPU was 1,715.08 ms, projection caller CPU 359.13 ms, runtime
construction caller CPU 5.02 ms. These caller components omit their workers and
are not a partition of server CPU. The five cohort cycles repeated 80 store opens
and 160 projections; active ten unrelated writes produced 160 projections and
zero output frames. This exposes duplicate work without adding a projection
broker or claiming that sharing it is safe.

The five-second 16-client stack sample contains
`Reader::step → open_read_only_store → SqliteStore::open →
configure_write_connection → sqlite3ReadSchema → sqlite3InitOne →
sqlite3Prepare`, including SQLite allocation/mutex waits (sample lines
1208–1238). It also contains query compilation/execution and parked Tokio
workers. `sample` counts parked threads too: its totals are not CPU percentages
and do not quantify the runtime's share. Full profiles, including the 18 MB
16-client sample, remain in `default-profile/`.

All four default/profile samples balanced reader runtimes, runtime threads,
wake threads and SQLite opens/closes, and restored warmed FDs. At 16 clients,
threads were 20 warm, 323 with the cohort, 50 drained: the shared server retains
its grown blocking pool until shutdown. Post-cleanup SQL/projections were zero.
The unprofiled 16-client post-cleanup window nevertheless used 371.14 ms CPU
(37.32 user, 333.82 system) over 2.2 seconds with no counted stage activity;
that tail remains unattributed. The one-client observer retired during its
post-cleanup window. Zero SQL does not mean zero CPU or immediate observer
retirement. Each unprofiled writer retained ten capture receipts (2,690 payload
bytes), with 92.99/91.69 ms recording API CPU, separate from server and child CPU.

Revised ownership proposal, draft, superseding the preserved design's borrowed
server Handle: a reader-owned current-thread Tokio runtime drives synchronous
pulls and owns its independent SQLite blocking pool. The two-slot saturation
fixture demonstrates progress with that mechanism while the outer server pool
is fully occupied. Snapshot/config/checkpoint/error ownership stays per reader;
runtime destruction remains off async workers. It removes per-reader async
worker pools, not runtime objects or nested SQLite workers. Predicted default
thread reduction is 16 async workers per live reader; SQL/projection counts
remain unchanged. The old “zero reader-owned runtimes” criterion therefore does
not apply to this alternative; the whole-cycle CPU/latency thresholds still do.
This is a tested ownership alternative, not a selected production patch. Its
concurrency effects need a separate controlled comparison before implementation
is justified. Borrowing the outer pool remains invalid regardless of its limit.

Exact separate profile command (completed):

```sh
uv run python /tmp/subscription-experiment-20261109/default-runner.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-experiment-20261109/default-profile --default-config --counts 1,16 --multiplexed-only --rounds 1 --profile
```

Remaining measure command, for parent step 4, replicates the default baseline
before deciding whether a concurrency experiment has a credible target:

```sh
uv run python /tmp/subscription-experiment-20261109/default-runner.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-experiment-20261109/default-repeat --default-config --counts 1,16 --multiplexed-only --rounds 5
```

All original 1/16/64 samples remain in `baseline/`; the new runner can also replay
that capped matrix against any later candidate. Source checkpoints are
`e7061670d` and `17c304389`. No production execution policy changed. Focused
checks above passed; queued-cancel acceptance remains a documented failure.
Broader acceptance and any alternating candidate comparison belong to parent
measure/gate. Task/Process shapes, distinct Task demand, OS wakeups, liveness/Git
child counts and installed-server CPU remain unmeasured. Change delivery timing
starts before control-socket dispatch, not at the writer's exact commit.

Context preservation conflict: the original scratch already exceeded the
12,000-token/98,304-byte targets. The instruction to preserve every pre-existing
scratch file takes precedence over trimming it. Only this new note and the new
`subscription-measure.html` were edited; the original design, questions,
research, measure.html and review remain byte-for-byte unchanged. Current size
is recorded after the final query below; limits were not raised.

Final verification: all 16 pre-existing scratch hashes match, both original
checkpoints remain ancestors, both frozen binary/runner pairs match freeze.json,
and no fixture server/writer processes remain. The final context query is saved
at `/tmp/subscription-experiment-20261109/context-final.txt`; before this receipt
scratch exceeded targets by 35,365 tokens and 113,102 bytes. The preserved source
notes account for most of that conflict. `subscription-measure.html` includes
all capped phase/resource receipts and both unprofiled default receipts.

## Step 4 measurement follow-up

[Repeated normal-default baseline and bounded Task/Process pilots](subscription-followup.html)
([commands and limits](subscription-followup.md)) retain every result above.
Five samples per Session shape now measure a median 6,551.27 server CPU ms
(3,909.75–7,841.11) for sixteen readers over ten seconds of unrelated activity,
160 projections and no changed frames. The new focused workload omits old churn;
its samples are not pooled with those above. The reproduced cleanup tail comes
from SQLite schema/trigger destruction after SQLITE_TRACE_CLOSE increments the
close counter. The original 371 ms run was unprofiled. Five bounded TaskGet/
ProcessList pilots passed; their n=1 timings are not speed comparisons.

Next hypothesis: one complete reader per identical server demand. No production
change or runtime-only candidate was made. Queued setup cancellation and orphaned
TaskUpdates candidates remain open. Frozen driver/fixture evidence and all new
receipts are in `subscription-followup-receipts.zip` and
`/tmp/subscription-measure-20261109/`; old checkpoints/reports remain.
