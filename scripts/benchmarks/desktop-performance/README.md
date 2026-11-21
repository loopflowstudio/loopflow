# Desktop performance

```sh
# Read timing records from ordinary launches.
uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json
uv run python scripts/benchmarks/desktop-performance/timings.py --html /tmp/loopflow-performance.html
open /tmp/loopflow-performance.html

# Observe the running app without sending input.
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --process "Loopflow Dev"
uv run python scripts/benchmarks/desktop-performance/record_live.py summarize /tmp/loopflow-live-recording

# Compare native workspace journeys on the same host and build mode.
cargo build -p loopflow --bin lf
uv run python scripts/desktop_performance.py run --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --output /tmp/desktop-after --baseline /tmp/desktop-baseline
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

Desktop and phone use the shared `Interaction` record: input → acknowledged →
usable → outcome. Desktop writes daily files under
`<Home>/desktop-cache/timings/interactions/`; phone uses its application-support
`performance/interactions/`. Writes run on a utility queue, capped at 16 MiB per
UTC day and fourteen daily files. Records contain build revision, launch identity,
thermal state and cached live Session count where available. No new CPU sampler
runs on the interaction path. Export journals before retention expires for trends.

```sh
uv run python scripts/benchmarks/desktop-performance/timings.py --html /tmp/loopflow-performance.html
```

The HTML starts with one bar chart per workflow and one bar per repository, showing
median input-to-usable time on a labeled scale per workflow, shared across repositories and builds. Build and capture sources remain
separate. Expand the details below the charts for acknowledgment/usable medians,
P95, individual attempts, outcomes and unmeasured workflows. P95 needs twenty completed samples. Failures,
superseded, cancelled, unmeasured and unfinished attempts remain counts. Unknown
revisions stay per-launch. App-state and commit callbacks cannot supply frame
latencies. Desktop uses ScreenCaptureKit to acquire its visible window; phone uses
ReplayKit's next composited video frame after readiness. Desktop capture supplies
a **visual-observation upper bound**, including native acquisition cost; phone
uses the frame's host-clock timestamp. Neither claims the first compositor
presentation timestamp. The table keeps these observations separate from
presentation-callback observations.
Endpoint validation receipts may live in `<Home>/desktop-cache/timings/interaction-validations.ndjson`:
`{"observation":"UUID","verdict":"rejected","reason":"why the endpoint is invalid"}`.
Rejected attempts retain their original outcome and reason in the report and stay
outside latency statistics. Acceptance never promotes an unavailable frame, dirty
build or unfinished attempt. Slow observations are not rejected for being slow.

Images are discarded immediately and never written to the journal. A Task metadata
pane or a read-only Session is not conversation input readiness. Superseding an
interaction prevents its pending capture from completing the replacement.

App launch records process creation → restored conversation input. The initial
journal row survives a launch that never shows a window; the first visible
restored window supplies acknowledgment and repository identity, and only its
focused input-capable terminal supplies usability. This excludes OS time before
process creation and is labeled **App process start → typing**, not launch-request
latency. Sidebar rows alone cannot finish it.

Task selection and Session return follow the exact prepared Session, including
already-focused retained terminals. New independent conversations follow their
new shell tab to its recorded, available Session. Refresh measures the original
Refresh click through reattachment to focused usable input. File selection ends
when the selected text snapshot is mounted, visible and editable. It does not
wait for a second click to focus the editor. Read-only patches do not qualify.

Send → response observed is currently instrumented for native Codex. A bounded
provider preview identifies the response, then visible terminal text is checked
at most ten times per second. Native clients use the existing watched transcript
reading; its publication can follow the full message. Streaming clients can
publish once after the first response word. Sources remain identified as
`codex_transcript` and `codex`. A new provider turn and
response text absent before submission are required before acquiring a frame.
This is an observed-response upper bound, including receipt delivery, polling and
capture; it is not the precise first compositor frame or bare model TTFT. Prompt
echoes, old-turn previews, tool output and spinners cannot qualify. Preview and
screen text stay in memory and are never added to performance records. Other
providers remain unmeasured until they supply the same response observation.

Start → typing begins at the Start button's original event (both New Task and an
existing Task), follows creation/preparation to the correct Session, and waits for
the focused terminal to accept input and acquire a frame. Complete → row gone
requires both a successful completion command and an observed removal of that
Task's row, in either arrival order. Neither server return nor a filter change by
itself finishes the journey. Failed commands remain failed attempts. Native context
menu actions can arrive without an AppKit event; these stay unmeasured. Use the
visible Complete button in Task & workflow for a timestamped completion trial.

The report shows stall counts at 50/100/250 ms and the longest busy turn across
the retained journal window. Each workflow also counts stalls overlapping its
measured attempts, matched by launch/build and original-input-to-outcome time.
Missing timestamps leave attribution unavailable. These are busy-turn counts,
not a scroll frame-drop measurement.

Server creation traces contain `task_creation` and timed `creation_step` spans:
repository resolution, lock, store open, receipt check, admission, held
conversation start and the answer's reads. The conversation logs
`conversation_setup` spans (`step=checkout|graph`); what follows them in a
sample is the provider starting. Both keep the creation request ID and elapsed
time in `<Machine>/logs/lf.log` under the `loopflow::start_timing` tracing
target, with no request text, title or provider output.
New Task creation uses the interaction observation ID as its creation request
ID, so `timings.py` nests the server and setup steps under their Start → typing
sample (`machine_steps` in `--json`, medians under the sample in text). A remote
Machine's steps stay in that Machine's log. Start records the Machine kind
(`local` or `remote`) and the report splits Start → typing by it.

Each record carries the selected repository (Desktop path or phone repository ID)
so Loopflow, Manabot and Kata stay separate. Session counts describe the cached
repository reading, not a sampled Machine-wide concurrency estimate.

On Desktop, **Help → Enable Visual Timing…** requests macOS screen-capture access.
Without an existing grant, a visible window, or a complete capture, the attempt
stays unmeasured. The app never requests access from an ordinary interaction.
Programmatic changes have no synthetic input and do not trigger capture.
On phone, **Machine → Start visual timing** starts an explicit ReplayKit capture
session with camera and microphone disabled. Stop it from the same menu; entering
the background also stops it. Only video timestamps are consumed. Stale frames,
failed capture and disabled sessions stay unmeasured; no screen content is saved.
Terminal readiness prepares a terminal draw before observing the next screen frame.

Phone's UIKit application entry point scopes the original UIEvent through native
dispatch. Explicit filter/navigation handoffs retain it across later model Tasks;
scene activation still reaches SwiftUI's scene-phase environment.

Desktop actions capture the AppKit event timestamp before asynchronous work, so
queued main-thread time is included. Programmatic work has no synthetic input.
Kernel process start is a launch diagnostic, not the launch-request timestamp.
Agent clicks provide stimulus; the app journal must supply the measurement.
The earlier three-repository pilot includes accessibility/screenshot observer cost
and cannot establish a reactivity baseline. The independent
[agent-input recorder](agent-input.md) retains its diagnostic receipts; its wall
intervals do not enter the headline table.

Work receipts count attributed CLI reads, served reads/writes and projections, with SQL
statement/row totals from one-off reads/writes and subscription setup/pulls. SQL scopes
follow inherited workers and reused server connections. Each evaluation has an ID,
so an interaction counts shared production once. No SQL text or row values leave
the server. Ordinary CLI invocations keep request tracing off.

`sqlComplete` is false while a requested receipt is pending or unavailable; its
SQL totals stay null. These attempts cannot supply a complete work-count gate.
Counters freeze at the terminal observation. Server protocol 6 carries receipts;
protocol 5 still serves the app, with SQL attribution marked unavailable.

App bundles stamp their source revision on Desktop, phone and development builds.
Dirty or unknown sources remain per-launch and cannot prove a before/after change.

The existing DesktopVitals run-loop observer records busy main-thread turns of at
least 50 ms through the same daily journal. Its existing hang watchdog still logs
long hangs; there is no second watchdog thread. Busy-turn durations do not measure
scroll/streaming frame drops. The report retains raw stall receipts.

Internal launch/read/frame timings stay under diagnostics, grouped by build
revision. Slow frame wait/apply samples include only readings selected at 100 ms
combined latency. Their distribution excludes fast readings. No load correction
or controlled-comparison claim follows from ordinary-use observations. The
installed app needs this build to produce the new journal; it cannot reconstruct
historical measurements.

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

The ordinary run uses DTO fixtures and `/bin/cat`. Snapshot runs and
`verify-fixture` use the supplied `lf` to create a Task and a resumable Session
in a private `LF_HOME`. A stand-in `claude` echoes input without an account or
network access.

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

## Compare each reactivity change

Collect the baseline on the parent revision and the candidate on the same quiet
host. Use identical measurement sources, fixture populations, provider stubs,
build mode, sampling and recorder settings. Archive the full directories outside
the checkout. Different application/CLI source builds are expected; changing the
measurement contract requires collecting both sides again.

```sh
uv run python scripts/desktop_performance.py run --optimized --samples 21 --output /tmp/reactivity-parent
# Check out and build the candidate, retaining the same harness and fixtures.
uv run python scripts/desktop_performance.py run --optimized --samples 21 --output /tmp/reactivity-candidate --baseline /tmp/reactivity-parent
uv run python scripts/desktop_performance.py report /tmp/reactivity-candidate --baseline /tmp/reactivity-parent
```

Review every scenario's failures and P50/P95 deltas in `report.md`, with raw
attempts in `attempts.jsonl` and build/host identity in `run.json`. Positive
deltas are slower. A complete report establishes that these journeys ran, not
that the whole app is faster. No automatic regression budget is scored. An
unavailable comparison, failed observation or missing P95 is an open measurement,
never a passing performance gate. Preserve `preflight/preflight.json` for rejected
runs too. A post-build preflight qualifies only the period before the trial;
thermal state, swap and unrelated whole-trial load still require review.

The committed-write fixture changes normalized `tasks` rows and uses the normal
revision triggers. Its `home-copy-v3-task-rows` contract replaces the obsolete
planning-JSON fixture; old fixture results cannot be compared with this one.

## Reactivity measurement coverage

```sh
# Run on the measured host before collecting a comparison. Retains a rejection;
# never stops unrelated work or requests native permissions.
uv run python scripts/desktop_performance.py preflight --output /tmp/lf-preflight

# Headless supporting measurement; includes 10,000 synthetic Tasks and Sessions.
# Record host preflight and build mode beside this output.
swift build --package-path swift --build-tests --jobs 4
uv run python scripts/test_network.py --desktop env LF_WORKSPACE_PERF_OUTPUT=/tmp/lf-projection.json swift test --skip-build --disable-sandbox --package-path swift --filter WorkspacePerformanceTests

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
| A3/LF1 | New Task/thread → state, visible submitted text, input ready separately | Owned disposable request; both | Input recorder; automated new-Task surface driver pending on both hosts |
| A4 | Submit → first visible word | Fixed replay bytes and cadence; both | Owned provider stub exists; replay-to-first-word driver pending |
| A5/A6 | Five idle threads: footprint, foreground/background CPU and power | Owned five-thread fixture; both | `record_live.py` process/CPU; coalition footprint/power and phone attribution pending |
| A7/A8 | Five streams: memory and key → visible echo P50/P95 | Fixed simultaneous replay streams; both | Native PTY echo/input recorder; five-stream fixture and hitch driver pending |
| A9/A10 | Quit +60 s processes/memory; five → twenty-thread cost | Same owned process tree; Desktop/mobile lifecycle policy | Process ancestry recorder; exit/scale sampling pending |
| A11/A12 | Open/scroll/switch 2,000 messages; memory slope | Pinned synthetic transcript, 30-minute soak; both | Native scroll/soak recorder; 2,000-message fixture pending |
| B1–B4 | Hello, request anatomy, fixed code question, repo-rule canary | Same model/config as bare CLI control; Desktop/helper/mobile | Usage/context fixtures; side-call token capture/redacted categorization pending |
| C1 | Orchestrated build → correctness, elapsed time, tokens | Pinned four-work-item repository; helper/Desktop | Graph fixtures; scored benchmark workload pending |
| C2–C6 | Follow-up delivery, composer lock, killed agent, network loss, restart | Owned replay processes and fault points; both | Transport/creation/recovery tests; native fault journeys pending |
| LF2 | Complete/reopen/move/claim → state, membership/count pixels, committed reconciliation | 10k Tasks, held/reordered responses; both | Domain receipts and model tests; native driver pending |
| LF3 | Cached/recent A→B→A → highlight, retained content, validated input | Retained/cold/evicted workspace; both | Native `session_return`, input recorder, navigation signposts; phone retention and per-gesture correlation pending |
| LF4 | Wave disclosure → local route/arrow, visible children | Warm/cold/held child reads; Desktop, phone picker | Hierarchy native journeys; actual disclosure/pixel correlation pending |
| LF5 | File/layout save, review/delivery → local feedback and confirmed result | Base-revision conflict, held writer; both where supported | `file_edit_refresh`, layout/PR fixtures; generalized operation projection pending |
| LF6 | Multi-window consistency; remote background/reconnect | Two windows/devices, epoch reset and lost reply; both | Transport fixtures; multi-window native journey pending |

Numeric experience budgets await a week of event-to-presented-frame live data.
The quiet-host comparison workflow below remains available but parked: it has
not produced a qualified baseline. Interleaved parent/candidate runs on the same
loaded host will use the app journal, retaining each run’s conditions. Correctness gates require local
selection/disclosure before held I/O, exact-identity attachment, no duplicate send,
and operation-local rollback. Headless projection time is not gesture or paint time.

`LF_PERF_OUTPUT` adds aggregate SQLite profile milliseconds and the number of
profile callbacks, without SQL text or values. Zero elapsed milliseconds can be
real because SQLite profile durations have millisecond resolution; callback count
proves that profiling ran. SQLite profile time may include waits inside SQLite and overlap nested
statements; it excludes Rust mutex wait and planning/Git/provider time. It does
not diagnose a lock bottleneck. Git Trace2 and provider milestones remain separate.

Separate repository discovery, Rust mutex waits, planning and provider calls from
SQLite profile time when collecting backend traces. A microbenchmark cannot
establish the latency of the native interaction.
