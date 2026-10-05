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
a baseline and a candidate.

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

The native runner retains three owned `cat` PTYs while navigating and refreshing
8- and 256-Task fixtures. It exercises filtering, scrolling during refresh,
Session return with an unsubmitted terminal draft, Monitor, split/zoom restore,
the Task details breadcrumb/sheet, Flow nodes, complete history disclosure
(one record per Task), and browsing/editing in the file sidebar. These use mounted
workspace controls and an owned temporary checkout. A fresh `cat` PTY is created
and verified each round. Its startup is not provider startup;
retained surface equality is not native provider identity proof.

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
transport rejects mutations. Reports distinguish completed harness execution
from acceptance: bitmap/OCR and PTY echo do not measure compositor paint or
key-to-glyph latency. Baseline comparison requires the same host, measurement
sources, fixture contract and soak duration. The September 24 receipt is kept
but is incompatible with the expanded harness.

LOO-371 owns the representative snapshot runner for real CLI comparisons. Its
October 5 committed tree and checkout contained no such runner, so this harness
supplies no real-snapshot CLI receipt. `launch.py`'s database backup alone does
not isolate copied checkout/process references, credentials or external writes;
it is not a substitute for that handoff.

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
Desktop's Session store, public connect command and mounted returned launch
command across client exit/relaunch. The owned provider checks its resume ID and
reads only its owned transcript; input replies establish readiness. The test
verifies one stable Loopflow Session and byte-identical native history on every
reopen. Headless `DesktopNativeSessionTests` exercises the same fixture and public
API without Ghostty when `LOOPFLOW_TEST_NATIVE_FIXTURE` names the runner's
`native-fixture.json`. No live conversation or credentials are used.

These CLI totals cover fixture setup and native reopening, not the DTO-backed
planning refresh or the retained-cat soak. Native provider service costs and
realistic workspace process/query volume still require LOO-371's isolated
snapshot runner; this fixture is not a replacement for that runner.
