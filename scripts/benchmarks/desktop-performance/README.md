# Desktop performance

```sh
# Every real launch on this machine, already recorded: samples, median, p95, failures, by app version.
uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json

# Real usage on this machine: record the running app for a minute, then read the report.
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --process LoopflowMac   # dev build
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --no-xctrace           # signposts + RSS only
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --template 'Time Profiler'   # attribute main-thread hangs; open hitches.trace in Instruments
uv run python scripts/benchmarks/desktop-performance/record_live.py summarize /tmp/loopflow-live-20260926-1200

# Real launches of the release build against a private copy of your Home; nothing live is touched.
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/desktop-launch --output /tmp/desktop-launch/run

# Startup without a display: replay captured `lf` reads and their latency in-process.
uv run python scripts/benchmarks/desktop-performance/startup.py capture --repo ~/src/loopflow --output /tmp/startup-capture
uv run python scripts/benchmarks/desktop-performance/startup.py run --capture /tmp/startup-capture --output /tmp/startup-run

# Native capture/OCR journeys; the historical September receipt remains on disk.
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-after
# Capture/OCR journeys: record a baseline before changing the production build.
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-before
uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/desktop-after --baseline /tmp/desktop-before

# Preserve a realistic Home, then exercise Task links through Podium and local CLI reads.
uv run python scripts/desktop_performance.py snapshot --database /path/to/Home/loopflow.db --output /tmp/task-snapshot
uv run python scripts/desktop_performance.py run --snapshot /tmp/task-snapshot --lf /path/to/lf --repo /path/to/repo --issue LOO-368 --samples 21 --output /tmp/task-before
uv run python scripts/desktop_performance.py run --snapshot /tmp/task-snapshot --lf /path/to/optimized-lf --repo /path/to/repo --issue LOO-368 --samples 21 --output /tmp/task-after --baseline /tmp/task-before
```

The app appends to `<Home>/desktop-cache/timings/launches.ndjson` and
`reads.ndjson` whenever it runs outside a test mode; `timings.py` only reads
them. Each file keeps its newest half past 256 KB. Lines hold durations, `lf`
subcommand words, the workspace part and the app version: no arguments, output
or error text. Delete the directory to start over.

| Row | Milliseconds from kernel process start to |
|---|---|
| `pre_main` | `LoopflowApp.init` |
| `restored` | the saved workspace looked up (`saved workspace: hit/miss`) |
| `first_frame` | the first window's content committed to the render server |
| `usable_saved` / `usable_fresh` | outline rows observed, then the next main-queue callback; split by whether any part was saved text |
| `fresh` | every part of the workspace read by this launch |

`lf read` and `refresh` rows are durations of one subprocess read and one
planning or Sessions refresh, with failures counted apart. Reads have no
timeout, so a hung read shows as a launch that never reached `fresh`. A launch
still running, or quit early, is counted the same way. `first_frame` is a
commit, not on-glass presentation; CPU, memory and main-thread stalls are
`record_live.py`'s.

Snapshot runs preserve every database row and use a fresh private copy for each
invocation. Keep snapshots outside Git: they contain private history and credentials.
Only local read commands run against copied records; provider connection and
execution are refused. Repository files and placement metadata still come from
the supplied repository, so preserve those inputs across comparisons too.
Use the same host, CLI build mode, snapshot and measurement source before/after.
Both runner modes build their native tests and launch the selected test directly
so interruption can stop its process group. Allow disk/build capacity first.
Archived SwiftPM-launcher runs remain historical evidence; the changed command
requires fresh baselines for comparisons.

The endpoint is a native bitmap with recognized Task identity, not OS application
launch, compositor presentation or usable Session input. Read timings and sampled
window/focus/sheet transitions are in `attempts.jsonl`. The
[October 4 evidence](20261004-task-open/README.md) records failed attempts,
overlapping-run discovery and the budgets' origin; it establishes no speedup.
The [October 5 evidence](20261005-task-open/README.md) compares base and branch
on one snapshot: warm and reopen meet 250 ms, cold misses 5,000 ms behind one
`lf` read. The runner changed afterward, so new runs need a fresh baseline.

`record_live.py record` attaches to the app you are already using (`Loopflow` from
/Applications, or `LoopflowMac` from `swift/.build`), waits `--seconds`, then writes
one directory: `signposts.ndjson` (the app's own `studio.loopflow`/`perf` intervals
from the unified log), `hitches.trace` plus exported `hitches.xml` and
`potential-hangs.xml` (Instruments' Animation Hitches template attached to the pid),
`rss.jsonl` (one `ps` RSS/CPU sample per second), and `report.md`/`report.json` with
per-interval p50/p95/max, superseded counts, hitch ms/s, hangs, RSS growth and
sampled CPU percentage. p95 requires twenty samples; older RSS-only receipts
report CPU as unmeasured. CPU is the app process's `ps` percentage, excluding
children, rather than CPU time accumulated over the recording interval.
Nothing is sent anywhere; delete the directory when done. Use the app normally
while it records — the intervals are named after what you did.

| Interval | Begins | Ends |
|---|---|---|
| `cold_start` | `LoopflowApp.init` (message carries `pre_main_ms`) | next main-queue callback after non-empty rows are observed |
| `workspace_current` | `LoopflowApp.init` | every part of the workspace was read by this launch, none shown from the saved workspace |
| `hierarchy_interaction` | fold, presentation, filter keystroke, repository switch | next main-queue callback after changed rows are observed |
| `task_workspace_ready` | `scenario=wave` / `task`: selection; `scenario=session`: Session opened | next main-queue callback after detail/focus is observed |
| `retained_workspace_action` | collapse, expand, focus or restore of a retained pane | next main-queue callback after the workspace observes the layout change |
| `lf` | one `lf` subprocess read (message = verb) | stdout decoded |
| `markdown_parse` | one Description/Comment parse | blocks built |
| `terminal_key_to_draw` | key press in a Ghostty pane | next display-link draw |

The navigation intervals end in a queued main-thread callback. They measure
scheduling after the view observes a change, not completed layout, drawing or
presentation. No one-frame presentation bound is established. Instruments
measures hitches and hangs separately for the same period. An interval ended `superseded` means a newer interaction replaced it
before it finished; the report counts those separately for each scenario, even
when none finished. Missing trace tables are unmeasured (`null` in JSON), not
zero hitches or hangs.

`xctrace` needs Xcode; `--no-xctrace` still gives intervals and RSS. `leaks <pid>`
against the signed app reports "not debuggable" and sees read-only memory only;
for a full leak graph use the dev build.

An in-process XCTest harness measuring the same intervals on fixture data
(`XCTOSSignpostMetric`, clock and memory metrics, 4 Waves × 50 Tasks) is
LOO-300's work; it is not in this tree.

`20260924-capture-input/` is the earlier bitmap-capture/OCR baseline for
`scripts/desktop_performance.py`; its README explains why it is not comparable
with signpost intervals.

`launch.py run` copies the Home's database with SQLite's backup, builds the
release binary into its own app bundle (`com.loopflow.mac.bench`), and opens it
in the background: three launches with no saved workspace, twenty with one, and
three whose planning and Sessions reads fail. The bundle's `lf` forwards the
startup reads to the installed `lf` under the copy and refuses every other
command, so no helper, Session or repair starts. Each sample is the app's own
launch journal plus `ps` RSS and CPU time at the endpoint and the `lf` processes
it started. `--built` reuses the bundle already in `--work`; `--home <other
work>/home` copies another run's Home, so a baseline and a candidate read the
same data. It needs a logged-in desktop; OS file caches stay warm, and
main-thread stalls are `record_live.py`'s. `20261004-launch-rendered/` compares
a baseline and a candidate; `20261005-first-render/` alternates the two in
rounds so both see the same host load.

`--first-launch N` (three by default) opens N copies of the bundle, each at a
new path and each once, with a saved workspace. The system charges a binary it
has not run before, so this is the launch after an update; the other scenarios
reopen one bundle and never pay it. `--strip` builds the bundle without local
symbols. `20261005-first-launch/` is the receipt: about 390 ms more before
`main`.

`startup.py` measures when the model first holds outline content for an
uncached launch, a launch with a saved workspace, a saved launch whose reads
fail, and a second window in the process that saved. Twenty samples by default,
with `lf` reads counted before usable and until settled. It runs without a
display, so it reports no first frame, CPU, memory or stalls;
`20261004-startup-inprocess/` is its receipt.

## Unattended workspace journeys and soak

```sh
uv run python scripts/desktop_performance.py run --cli target/debug/lf --samples 21 --soak-seconds 3600 --output /tmp/workspace-baseline
# Run the same harness on the candidate, on the same display host.
uv run python scripts/desktop_performance.py run --cli target/debug/lf --samples 21 --soak-seconds 3600 --output /tmp/workspace-candidate --baseline /tmp/workspace-baseline
```

The [workspace journeys](../../../performance/README.md) use mounted controls,
an owned temporary checkout and three retained `cat` PTYs. Synthetic planning
and active-Session DTOs supply 8 Tasks/4 Sessions or 256 Tasks/128 Sessions, with
one explicit history record per Task. A fresh `cat` PTY is created each round;
its startup is not provider startup. Snapshot mode adds native reopening in
the combined workspace described below.

After the large-population journeys, the soak alternates thirty seconds idle
with Session switching, typing/echo and split focus/restore for the requested
duration. The production `keepWorkspaceCurrent` owner supplies refresh cadence.
Every round checks retained surfaces, layout, Session records and the file draft.
The native journal records phase times, fixture-read counts, window counts and
focus endpoints. The existing live recorder attaches to that test PID for CPU,
RSS, signposts and optional xctrace evidence. The report includes RSS change at
the fourth soak round. Idle hitch/hang analysis aligns phase timestamps to the
trace's exported start/end dates and reports actual coverage, clipping intervals
at idle boundaries. Missing clock metadata, trace tables or resource reports
stay unmeasured; partial idle coverage does not establish the budget. Interrupted or shortened soaks and
failed preservation checks cannot produce a complete journey report.

These fixtures never open a configured Home, provider or real Task checkout.
The runner removes inherited `LF_*` context, uses a private Home, and fixture
transport rejects mutations. The journey report's comparison contract also
requires matching soak duration. Bitmap/OCR and PTY echo do not establish
key-to-glyph latency. The September 24 receipt remains on disk but is incompatible
with the expanded harness.

```bash
uv run python scripts/desktop_performance.py verify-fixture --lf target/debug/lf --output /tmp/desktop-fixture-proof
uv run python scripts/desktop_performance.py run --snapshot /tmp/task-snapshot --lf target/debug/lf --repo /path/to/repo --issue LOO-368 --samples 21 --soak-seconds 3600 --output /tmp/desktop-soak
```

Snapshot mode uses LOO-371's real CLI transport and Task links. Its final mounted
workspace also contains a Task-bound native fixture from a fresh Home. The test
transport joins read results without changing copied identities, histories or
paging cursors. Only the owned Session can connect; its Task membership comes
from the public bind command and shared Rust reader. Both Homes' CLI work appears
in scenario volume. The added fixture and sandbox are measurement overhead, so
baseline and candidate must use this same harness.

The combined soak reopens that Session through its Task link, types and checks a
reply, navigates away/back, refreshes a dirty file, and retains the companion pane,
selection, transcript and native identity. Podium's existing refresh owner keeps
snapshot reads running during idle. Fixture mode still covers the broader
synthetic catalogue; it no longer opens a separate native-session window.

`verify-fixture` builds the Swift tests and exercises real snapshot reads, Task
routing, repeated native reopening, owned temporary files/PTY allocation and
draft/pane preservation with WindowServer access denied. It also probes external file reads/writes, provider execution,
scheduling-tool execution and network binding. This is a small synthetic snapshot
proof, not a density, rendering, real-provider or hour-long acceptance receipt.

One OS sandbox, inherited by the native runner and every child, permits writes
only beneath the output directory, local sockets there and a fixed executable
set including the owned stub. Selected repository/source reads remain available; external credential files,
network access, signals and copied providers remain denied. Other copied checkout
paths can report unavailable reads; retain and resolve those failures before
scoring realistic-snapshot budgets. Denied work is not a performance improvement.
CLI environments select their own Homes explicitly. The app's Home is private;
copied workspaces remain observational. `desktop-performance.sb` is part of the
measurement-source hash. macOS rejects nested sandbox application; the runner
owns this single boundary. Invoke the proof through this script, not by running
the opted-in Swift tests directly.

### CLI volume and native reopening

```bash
mkdir /tmp/cli-volume
# Set this only on commands inside the isolated snapshot runner:
# LF_PERF_OUTPUT=/tmp/cli-volume <isolated CLI command>
uv run python scripts/desktop_performance.py volume /tmp/cli-volume
```

`LF_PERF_OUTPUT` observes actual CLI processes and their main-store SQLite
connections. Each process writes start, cumulative one-second samples and end to
its own receipt, without SQL, arguments or row contents. Counts include schema
validation, PRAGMAs, writes and triggers. Rows are emitted results, not scanned
rows. Non-CLI children and other SQLite connections are outside coverage. A
missing end or malformed tail is partial; no receipt is unmeasured, never zero.
Use the same instrumentation on baseline and candidate: row counters and the
writer add overhead. The output directory must already exist; observation never
selects a Home or grants execution authority.

The native runner collects these receipts under `cli-volume/` and uses `--cli`
for a fresh synthetic native Session fixture. `native_session_reopen` exercises
the Task link, Desktop Session store, public connect command and mounted
returned launch command across client exit/relaunch in the snapshot workspace. The owned provider checks its resume ID and
reads only its owned transcript; input replies establish readiness. The test
verifies one stable Loopflow Session and byte-identical native history on every
reopen. The headless proof exercises the same fixture and public API without Ghostty.
No live conversation or credentials are used.

Scenario receipts in `cli-volume/` cover native reopening; setup receipts stay
separate in `fixture-setup-cli-volume/` and `fixture_setup_cli_volume` in the report.
Both retain partial counts. Older recordings without separate setup receipts
remain unscoped. Fixture-mode counters do not measure DTO-backed planning refresh
or the retained-cat soak. Snapshot mode records real refresh and Task-link CLI
reads from both Homes throughout the combined soak. The combined path is
implemented; a matched rendered comparison and hour-long acceptance receipt
remain outstanding.

### October 6 rendering capability

The unattended one-sample probe on `Jacks-MacBook-Pro-2.local` produced native
bitmap/OCR output, but no completed journey. After repairing owned journal/temp
files and PTY allocation, the captured terminal reported that Ghostty's
`/usr/bin/login` launch was denied by the inherited sandbox. Explicitly allowing
that executable did not resolve the rejection; the ineffective permission was
removed. No renderer or latency budget passed. Local raw receipts remain under
`/tmp/loo304-render-probe-*-20261006/`; the final policy reproduces the denial in
`/tmp/loo304-render-final-20261006/`.

The four contained `verify-fixture` checks pass, including external-effect denial
and owned PTY allocation. They do not exercise Ghostty's mounted login path.
A disposable macOS rendering account or VM must demonstrate contained terminal
startup before running the matched snapshot pair (21 samples and 3,600 seconds
each). Keep snapshots local to that isolated host. No such remotely callable host
or performance job is configured in this checkout. Hosted `ci.yml` jobs
`swift-test` and `loopflow-ui-test` on `macos-15` own headless tests and the Xcode
compile check only; checkpoint PRs defer that matrix while scratch remains.
This remaining performance obligation does not prevent source publication under
Jack Heart's October 6 direction, and does not authorize merge or Task completion.

Source gate on October 6: the affected runner passed 2,250 Rust tests (20 skipped),
370 Python, 78 website (three skipped), 377 headless Swift tests, formatting,
Clippy and architecture/boundary checks. The separate standard Xcode
`build-for-testing` passed. The rebuilt source CLI passed all four contained
proofs at `/tmp/loo304-final-source-contained-20261006/`. Gate and Xcode logs:
`/tmp/loo304-gate-20261006.log` and `/tmp/loo304-xcode-fallback-20261006.log`.
These checks establish source behavior, not rendered responsiveness or installation.
