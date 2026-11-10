# Default subscription measurement — 2026-11-09

Optimize step 4; production stays at `17c304389` with kept repair `4d7de3695c`.
Claim under test: “At ordinary runtime defaults, sixteen identical Session
subscriptions repeat projection work on unrelated activity; their CPU cost is
materially greater than one subscription, while cancellation releases owned
resources.” This is a baseline claim, not a speedup.

New workload `focus-v1-session` keeps the frozen driver's cold warm-up, pure
runtime probe, admission/first reading/checkpoint, ten-second quiet and active
windows, title/exit delivery, cancellation and cleanup. It omits all long churn
and adds a second 2.2-second tail window. Five fresh processes per 1/16-reader
shape, alternating shape and window order. This new baseline never pools samples
with the original full-churn workload. All 25 capped and 40 observer samples
remain valid for their original scopes. Driver phase CPU is separate from
server and child CPU. The frozen Rust binary is unchanged; no build is needed.

Receipts and driver: `/tmp/subscription-measure-20261109/`. Original scratch
hashes are in its `preserved.json`. The new runner SHA-256 is
`5047a602a463f44d4e3b9cedd2f338b71c8f8dcd352c0361d90bfe8c561e2d70`;
server SHA-256 remains
`a8531bca7c88678b666ad1d793276f08c14bbb6230b44d8a30916fb6bf071413`.

```sh
uv run python /tmp/subscription-measure-20261109/focus-v1.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/session-baseline --default-config --counts 1,16 --multiplexed-only --rounds 5
```

TaskGet/ProcessList inspection will be separate bounded pilots, not a new full
matrix. Task pilots use actual isolated Git worktrees, completed Graph history
associated by checkout, and Task Graph rows; empty Tasks cannot represent this
path. Any fixture change starts a separately named baseline. Queued setup
cancellation is still a failing acceptance check; orphaned TaskUpdates
candidates stay with the next implement step.

## Repeated default baseline

Ten fresh-process samples passed, five per shape, reversing shape and quiet/
active order. Median (range), server user+system CPU:

| Readers | Admission + checkpoint | Active / 10 seconds | Quiet / 10 seconds | First 2.2 s after close counters balance |
| --- | ---: | ---: | ---: | ---: |
| 1 | 30.33 (26.94–48.66) ms | 103.07 (87.10–117.75) ms | 7.42 (5.02–7.69) ms | 0.26 (0.06–0.57) ms |
| 16 | 2,784.45 (1,687.52–4,642.09) ms | 6,551.27 (3,909.75–7,841.11) ms | 54.14 (52.54–58.16) ms | 377.85 (150.07–455.36) ms |

Every active window completed ten real recording-API writes, ten/160
projections and zero changed frames. Quiet windows made zero projections.
Reader-store opens were 1/16 during admission; traced SQLite connection opens
were 2/32. No new connections opened in active windows. Admission + checkpoint
made 2/32 projections. All normalized initial readings share one digest;
relevant title and unrecorded process-exit changes reached every subscriber.
No unexpected errors. FD counts returned to 17; runtime, runtime-thread,
wake-thread and connection counts balanced after shutdown. At 16 readers the
server retained idle blocking workers: 20 warm, 323–325 with readers, 35–38
at the drained resource inventory. This is not a reader-thread leak.

The second 2.2-second tail window used 0.026 ms median at both populations
(1 reader: 0.019–0.029; 16: 0.024–0.036), with zero SQL/projections.
Admission/checkpoint plus cancellation and both tail windows used 33.21 ms
(29.22–51.87) versus 2,971.22 ms (2,135.32–5,095.92). These are CPU totals,
not user-visible latency. Last initial reading latency was 42.4 ms
(33.1–63.7) versus 515.1 ms (469.9–542.7); last checkpoint latency was
10.6 ms (9.5–87.9) versus 122.1 ms (114.9–135.3).

Pure sequential runtime create/drop cost was 1.03 ms (0.77–1.50) at the
one-reader shape and 1.12 ms (0.81–1.34) at sixteen. Multiplying the latter
by sixteen gives ~18 ms, about 0.6% of the measured ~2,971 ms complete-cycle
CPU median. This component probe still does not bound concurrent interactions.
Removing fifteen of sixteen identical computations would remove 93.75% of
projection *count*, not necessarily 93.75% of CPU or latency. The active CPU
median is ~65.5% of one core; the workload is an instrumented fixture, not an
installed-server CPU reading.

Writer recording-API CPU was separate: 117.03 ms (100.34–129.51) and
109.12 ms (103.57–135.59) for ten writes. Server child CPU during admission/
checkpoint was 11.74 ms (10.14–16.53) and 255.70 ms (219.52–286.26);
active-window child CPU was zero. These are CPU deltas, not Git-child counts.
Resource-inventory `ps` costs remain separately recorded outside phases.
Driver phase CPU, server setup CPU and writer receipts remain in raw results;
setup SQL is uncounted because instrumentation starts after seeding. The
runner's setup-driver field is explicitly cumulative, not a per-sample setup
cost. Task enrichment records its own driver/Git-child setup costs separately.

## Cleanup attribution

Two separate profiles are excluded from timing ranges. The first attached too
late to capture the short teardown burst; its snapshots still recorded 59 ms
in the first 58 ms after the counter gate. It is not treated as an idle profile
that disproves the tail. The second attached 1.5 seconds before cancellation.
Its post-gate window used 414.30 CPU ms; 402.27 ms arrived in the first 54 ms,
with no added SQL, projections or close callbacks.

`focus-tail2-cleanup.sample:398–420` follows server `Held::drop` → subscription/
Reader drop → Store/Connection drop → `sqlite3Close` →
`sqlite3LeaveMutexAndCloseZombie` → `sqlite3BtreeClose` →
`sqlite3SchemaClear` → trigger/expression destruction. Following stacks include
SQLite allocation mutex lock/wake waits. This maps to
`server/service.rs:831`, `lf/commands/subscribe.rs:479`, and the bundled
SQLite `sqlite3.c:188452–188610`. The `SQLITE_TRACE_CLOSE` callback increments
`closes` **before** schema destruction; `runtime_drop` also completes before
Reader's later store field is destroyed. Therefore the current driver's
`_drained` predicate proves close initiation, not completed store destruction.
The repeated tail has an explanation; the original unprofiled 371 ms sample
cannot be assigned an exact stack retrospectively. Stack totals include parked
threads and are not CPU percentages.

Keep CPU accounting through the tail in candidate comparisons. A future
actual-reader-drop completion counter would strengthen acceptance but changes
the instrumented harness and needs a new baseline. The second tail window and
final shutdown checks establish eventual quiet/release here. No new production
cleanup code was added.

Profile reproduction (both use the same frozen server):

```sh
uv run python /tmp/subscription-measure-20261109/profile-driver-tail.py /tmp/subscription-measure-20261109/focus-v1.py /tmp/subscription-measure-20261109/reprofile /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/reprofile-results --default-config --counts 16 --multiplexed-only --rounds 1
```

The wrapper runs `/usr/bin/sample PID 5 1 -file PREFIX-cleanup.sample` and
captures 50 ms cumulative snapshots during the 2.2-second tail. Original actual
prefixes were `focus-tail` and `focus-tail2`; the first used the preserved
`profile-driver-v1.py` (0.15-second attachment lead). The exact reconstructed
wrapper versions and hashes are retained with the receipts.

## TaskGet and ProcessList: five bounded pilots

All five successful pilots use the unchanged server binary and `paths-v1.py` /
`path_support.py` hashes in `freeze.json`. One fresh process per row, **n=1**;
there is no stable timing range or same-versus-distinct speed verdict.

| Demand | Readers | Admission/checkpoint CPU ms | Quiet CPU ms / 10 s | Active CPU ms / 10 s | Quiet / active projections | SQLite connections opened at admission |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ProcessList | 1 | 12.48 | 2.73 | 9.65 | 4 / 11 | 3 |
| ProcessList | 16 | 3,562.27 | 46.20 | 90.16 | 74 / 166 | 48 |
| TaskGet | 1 | 65.81 | 72.57 | 143.64 | 5 / 10 | 4 |
| TaskGet, identical | 16 | 8,781.86 | 4,057.53 | 3,498.82 | 80 / 160 | 64 |
| TaskGet, distinct | 16 | 5,239.93 | 2,444.02 | 4,323.43 | 68 / 172 | 64 |

Task windows additionally consumed **128/273 ms** (one), **2,135/4,220 ms**
(identical sixteen), and **1,977/4,242 ms** (distinct sixteen) of reaped-child
CPU in quiet/active windows, separate from server CPU. ProcessList child CPU
was zero in those windows. Resource-inventory `ps` happens outside phases;
no child-launch tracer identifies commands/counts, so these are not Git-count
claims. Task setup's isolated Git children are charged separately to the
Python driver, outside server timing. The row above alone understates total
Task observation CPU if these child costs are omitted.

ProcessList is cheap once admitted despite duplicate projection counts; its
large pilot admission and cleanup costs are separate. TaskGet duplicates
material work even during quiet windows. Identical versus distinct Task
CPU differences are inconclusive at n=1 and under this Machine's observed
variation. The identical Task pilot spent more CPU in quiet than active despite
half the projection count; that inversion is unexplained without separate
profiles/repeats and is not a performance verdict. The distinct pilot recorded 68 quiet and 172 active projections, versus
80/160 for identical demand; twelve more completed during cancellation
after the relevant change;
all remain counted. No SQL/projections ran after its cleanup gate. This does
not repair or overturn the existing queued-setup cancellation failure.

Task enrichment happens once before timing: sixteen isolated real worktrees
on local branches, two existing completed Graph processes associated with
each checkout, a valid captured Task Graph, and a primary Session. Initial Task
readings validate identity, worktree, two Graph details, captured Graph and
Desktop row. Identical demand observed Task 0; distinct demand observed all
sixteen. Task 0's relevant title update reached all sixteen identical clients
(419–1,287 ms), or its single distinct client (295 ms). Other Task outputs stayed
unchanged in the unrelated-activity window. This represents completed Graph
history; it does not represent sixteen live Graphs or production-sized history.

ProcessList starts with live fixture AgentProcesses; after the sleeping fixture
client exits without a DB completion write, every client's list becomes empty
(1,596 ms at one reader; 1,617–1,624 ms at sixteen). No real provider is involved.
All pilot quiet/active windows emitted zero changed frames, all ten writer
receipts survived, FDs returned to 17, and runtime/thread/connection counts
balanced. Full initial bodies and fixture manifests are retained.

Calibration accounting: `task-calibration/` failed in the Python driver's
closure because its `kind` comprehension variable shadowed the demand kind;
there is no timing result for that failure. Renaming it fixed the driver.
`task-calibration2/` passed and supplies the one-reader Task pilot above; its
source hashes match the two sixteen-reader Task pilots. No retry or failed
sample is silently averaged into the table. The original queued-cancel test
remains one separate known acceptance failure.

Exact pilot commands (all `--default-config --multiplexed-only --rounds 1`):

```sh
BENCH_KIND=process uv run python /tmp/subscription-measure-20261109/paths-v1.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/process-pilot --default-config --counts 1,16 --multiplexed-only --rounds 1
BENCH_KIND=task-same uv run python /tmp/subscription-measure-20261109/paths-v1.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/task-calibration2 --default-config --counts 1 --multiplexed-only --rounds 1
BENCH_KIND=task-same uv run python /tmp/subscription-measure-20261109/paths-v1.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/task-same-pilot --default-config --counts 16 --multiplexed-only --rounds 1
BENCH_KIND=task-distinct uv run python /tmp/subscription-measure-20261109/paths-v1.py /tmp/subscription-experiment-20261109/default-server --output /tmp/subscription-measure-20261109/task-distinct-pilot --default-config --counts 16 --multiplexed-only --rounds 1
```

These are the actual Read dispatch variants seen in the original live-server
sample: `server.sample.txt:619–644` reaches TaskStatus/graph execution facts;
`:1636` reaches `subscribe_ps`. That old installed sample is not a timing
baseline for this checkpoint. Source mappings for the newly exercised paths:
`read.rs:829–840`, `desktop.rs:659–677`, `waves.rs:943–1006`, and
`top.rs:144–162,253–282`. The actual pilot work counts expose extra SQLite
connections, not a complete count of temporary Task-resolution runtimes.

## Recommendation and handoff

**Next hypothesis: one complete reader per identical server demand**, sharing
store/schema lifetime and computation while keeping each subscriber's delivery,
selection and checkpoints independent. The five repeated Session samples
support that experiment; the Task pilots add quiet projection and child work
that runtime-only ownership changes cannot remove. Distinct-demand performance
and narrower invalidation remain separate questions. This step selects no
concurrency cap and claims no production speedup.

Use the frozen focused Session driver for alternating baseline/candidate runs;
include both tail windows in cycle CPU. Retain the 25 capped churn and 40
observer samples as separate proof of their original workloads. The Task/
Process pilots are ready for targeted repetitions if the candidate touches
those paths; do not present them as n=5. Preserve checkpoint freshness,
independent cancellation, errors/recovery and provider ownership. Borrowing the
server pool stays rejected. The orphaned TaskUpdates candidate reproduction/
repair remains with the next implement step.

No production edits, new Task, project worktree, publication, installation,
provider input/restart or policy change occurred. Only private fixture worktrees
were created and automatically removed. No Rust rebuild or redundant test
matrix was needed. Step 3's ten focused tests/fmt/clippy results are retained,
not relabeled as rerun checks. Original scratch content is retained; measure
and experiment index additions link this report without replacing old results.

Verification: all 15 successful unprofiled receipts passed output/work-count and
resource checks; both excluded profiles exited; the failed driver calibration
log is retained. `git diff --check` passed, tracked source is unchanged, and all
four checkpoints remain ancestors. Original research and the prior subscription
HTML match their saved hashes; only the two original measurement indexes gained
additions. No fixture server, writer, sampler or client remains running.
