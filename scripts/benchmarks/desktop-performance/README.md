# Desktop performance

```sh
# Read timing records from ordinary launches.
uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json

# Observe the running app without sending input.
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --process "Loopflow Dev"
uv run python scripts/benchmarks/desktop-performance/record_live.py summarize /tmp/loopflow-live-recording

# Compare native workspace journeys on the same host and build mode.
cargo build -p loopflow --bin lf --bin lf-world
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-after --baseline /tmp/desktop-baseline
```

Keep recordings, snapshots and reports outside the checkout. Snapshots contain
private history and credentials. Use disposable Homes and matching source,
fixtures, populations and build modes for comparisons. A successful command is
not evidence of a speedup; incomplete attempts and failures remain in reports.

## Ordinary-use observations

The app retains bounded launch/read journals under
`<Home>/desktop-cache/timings/`. `timings.py` reads those journals without
launching the app. Records contain durations, command names and app versions,
not arguments, prompts or output.

`record_live.py` collects signposts, process RSS/CPU samples and, when available,
Instruments hitch/hang traces. `--no-xctrace` keeps signposts and resource samples
without Instruments. `--template 'Time Profiler'` selects another Instruments
view. Missing trace tables are unmeasured, not zero. p95 requires twenty samples.
Navigation signposts end after the view observes a change; they do not prove
completed layout or on-screen presentation. CPU samples cover the app process,
not its children.

## Isolated startup and workspace checks

```sh
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/desktop-launch --output /tmp/desktop-launch/run
uv run python scripts/benchmarks/desktop-performance/startup.py capture --repo /path/to/repo --output /tmp/startup-capture
uv run python scripts/benchmarks/desktop-performance/startup.py run --capture /tmp/startup-capture --output /tmp/startup-run

uv run python scripts/desktop_performance.py snapshot --database /path/to/Home/loopflow.db --output /tmp/task-snapshot
uv run python scripts/desktop_performance.py run --snapshot /tmp/task-snapshot --lf /path/to/lf --repo /path/to/repo --issue TASK-ID --samples 21 --output /tmp/task-before
uv run python scripts/desktop_performance.py report /tmp/task-before
```

`launch.py` builds an isolated app bundle and measures uncached, saved and failed
startup reads against copied state. `--built` reuses its bundle; `--home` selects
a prior private fixture. Native launches need a display host. `startup.py`
replays captured reader responses without a display and makes no rendering claim.

The owned Task and Session come from an `lf-world` one-task world built by the
`lf-world` beside `--cli`/`--lf`; its stub `claude` resumes the world transcript
and echoes input, so no account or network is involved.

Workspace journeys use mounted controls, bitmap capture and text recognition.
They cover hierarchy interactions, retained panes, Task details and file editing.
Session return also checks focus, draft submission and PTY replies. Capture/OCR
adds observer overhead; these endpoints are not compositor-paint measurements.
Use `--samples 1` for a short behavioral check and `--soak-seconds 3600` for the
longer recorder. Reports retain failed and interrupted attempts and distinguish
planned observations that never started. Source drift or mismatched populations
invalidates comparison. `report` reconstructs results after interruption.

Snapshots are copied before each run. Copied production records permit local
reads only; provider connection and execution are refused. If only indexes
differ from the candidate schema, prepare a separate fixture:

```sh
uv run python scripts/desktop_performance.py prepare-snapshot --snapshot /tmp/task-snapshot --schema-database /tmp/candidate/loopflow.db --output /tmp/task-compatible
```

Preparation verifies typed row equivalence and exact candidate schema. It does
not convert tables/history or migrate the installed store. Use the same prepared
fixture on both sides of a comparison. Build and disk capacity are prerequisites
for the native test runners, not performance results.

For focused input profiling, see [agent-input.md](agent-input.md).

## LOO-906 measurement coverage

```sh
# Run on the measured host before collecting a comparison. Retains a rejection;
# never stops unrelated work or requests native permissions.
uv run python scripts/desktop_performance.py preflight --output /tmp/lf-preflight

# Headless supporting measurement; includes 10,000 synthetic Tasks and Sessions.
# Record host preflight and build mode beside this output.
swift build --package-path swift --build-tests --jobs 4
uv run python scripts/test_network.py --desktop env LF_WORKSPACE_PERF_OUTPUT=/tmp/lf-projection.json swift test --skip-build --disable-sandbox --package-path swift --filter WorkspacePerformanceTests

# Prepared dynamic membership: one Move in a 10,000-Task manifest, 21 samples.
uv run python scripts/test_network.py --desktop env LF_MEMBERSHIP_PERF_OUTPUT=/tmp/lf-membership.json swift test --skip-build --disable-sandbox --package-path swift --filter ClientWorkQueryTests/testDynamicManifestMeasurements
```

The minimum inventory follows [ADEMark's methodology](https://github.com/zqkra/ademark/blob/main/METHODOLOGY.md)
and [test procedures](https://github.com/zqkra/ademark/blob/main/TESTS.md), retrieved
2026-11-14. Borrowed procedures are coverage requirements, not Loopflow results.
ADEMark's published v1 excludes C2–C6; its reliability methods are not published
passing results. The public site was unreachable from the research reader;
the public source supplied the methods. A subsequent direct HTTP fetch read the
site successfully. Source HEAD was `119a5f7b99247cffbe7f96bb2c36c03496cf3018`.

Use one pinned build/helper/provider/fixture per comparison, at least 20 successful
native observations per latency journey, plus every failed, interrupted or rejected
attempt. Keep warm/cold, deliberate five-agent load/idle, and foreground/background
separate. A quiet preflight observes 60 seconds below 0.1 core of pre-existing
process CPU, then checks display/capture/input access without prompting. It is a
host screen, not a whole-trial thermal/swap or contamination measurement. Retain
background CPU throughout native trials. Do not turn unrelated host load into the
five-stream fixture. Prefer a quiet mini that can run the app's deployment target.

| ID | Workflow and endpoint | Fixture/platform | Existing instrument; remaining gap |
|---|---|---|---|
| A1/A2 | Cold start/relaunch → visible accepted typing | Empty/saved owned workspace; Desktop, mobile launch/background return | `startup.py`, launch timings, input recorder; phone visible-input driver pending |
| A3/LF1 | New Task/thread → state, visible submitted text, input ready separately | Owned disposable request; both | Creation signposts/input recorder; mobile immediate destination pending |
| A4 | Submit → first visible word | Fixed replay bytes and cadence; both | Owned provider stub exists; replay-to-first-word driver pending |
| A5/A6 | Five idle threads: footprint, foreground/background CPU and power | Owned five-thread fixture; both | `record_live.py` process/CPU; coalition footprint/power and phone attribution pending |
| A7/A8 | Five streams: memory and key → visible echo P50/P95 | Fixed simultaneous replay streams; both | Native PTY echo/input recorder; five-stream fixture and hitch driver pending |
| A9/A10 | Quit +60 s processes/memory; five → twenty-thread cost | Same owned process tree; Desktop/mobile lifecycle policy | Process ancestry recorder; exit/scale sampling pending |
| A11/A12 | Open/scroll/switch 2,000 messages; memory slope | Pinned synthetic transcript, 30-minute soak; both | Native scroll/soak recorder; 2,000-message fixture pending |
| B1–B4 | Hello, request anatomy, fixed code question, repo-rule canary | Same model/config as bare CLI control; Desktop/helper/mobile | Usage/context fixtures; side-call token capture/redacted categorization pending |
| C1 | Orchestrated build → correctness, elapsed time, tokens | Pinned four-work-item repository; helper/Desktop | Graph fixtures; scored benchmark workload pending |
| C2–C6 | Follow-up delivery, composer lock, killed agent, network loss, restart | Owned replay processes and fault points; both | Transport/creation/recovery tests; native fault journeys pending |
| LF2 | Complete/reopen/move/claim → state, membership/count pixels, committed reconciliation | 10k Tasks, held/reordered responses; both | Domain receipts and model tests; shared projection/phone actions and native driver pending |
| LF3 | Cached/recent A→B→A → highlight, retained content, validated input | Retained/cold/evicted workspace; both | Native `session_return`, input recorder, shared interaction marks; phone retention pending |
| LF4 | Wave disclosure → local route/arrow, visible children | Warm/cold/held child reads; Desktop, phone picker | Hierarchy native journeys, synchronous selection marks; actual disclosure/pixel correlation pending |
| LF5 | File/layout save, review/delivery → local feedback and confirmed result | Base-revision conflict, held writer; both where supported | `file_edit_refresh`, layout/PR fixtures; generalized operation projection pending |
| LF6 | Multi-window consistency; remote background/reconnect | Two windows/devices, epoch reset and lost reply; both | Stamped-read/transport fixtures; shared cache and native journey pending |

The proposed budget is P95 <100 ms for local reaction, separately from visible
pixels and input readiness. Other regression budgets await a valid baseline;
never substitute a time limit on the runner. Correctness gates require local
selection/disclosure before held I/O, exact-identity attachment, no duplicate send,
and operation-local rollback. Headless projection time is not gesture or paint time.

`PerfClock` emits shared `interaction` marks for Desktop selection/completion and
phone Task tap/workspace mount. Marks name the phase; `state_published` and
`content_mounted` do not mean pixels appeared. Phone marks currently correlate by
Task identity, so repeated same-Task occurrences require native capture rather than
pairing ambiguous marks. Creation/input-ready signposts retain their request UUIDs.
`LF_PERF_OUTPUT` adds aggregate SQLite profile milliseconds, without SQL text or
values. SQLite profile time may include waits inside SQLite and overlap nested
statements; it excludes Rust mutex wait and planning/Git/provider time. It does
not diagnose a lock bottleneck. Git Trace2 and provider milestones remain separate.

The current creation admission span includes Git repository discovery for legacy
fingerprint equivalence. It is not a pure SQL-admission interval yet. Separate
that lookup from SQL lock/query time when collecting backend traces.
