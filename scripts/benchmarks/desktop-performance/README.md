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

# A commit from another process to the row on screen, through the real reader, on that private copy.
uv run python scripts/desktop_performance.py write-visible --home /tmp/desktop-launch/home --repo ~/src/loopflow --output /tmp/write-visible

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

Add `--optimized` to both runs for release-configuration Swift measurements.
The receipt records the build mode; compare matching modes only.

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
If a retained snapshot differs only in indexes, build a separate fixture using
an existing fresh candidate-schema database produced by `verify-fixture`:

```sh
uv run python scripts/desktop_performance.py prepare-snapshot --snapshot /tmp/task-snapshot --schema-database /tmp/ghostty-launch-proof/native-home/loopflow.db --output /tmp/task-compatible
```

Preparation opens both inputs read-only and creates a new database. It copies
every table row, SQLite row identity and sequence counter before restoring
triggers, then proves typed content equivalence and exact candidate schema.
Only indexes may differ; no table/history conversion or validation bypass is
supported. The private manifest retains source hashes and per-table digests;
it contains no row payloads. Use the same prepared fixture for both variants.
This is fixture preparation, not an in-place migration or runtime authority.

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
window/focus/sheet transitions are in `attempts.jsonl`. Each observation's
`steps` split its wait into observer wake, layout and display, and record
whether planning had been published: OCR holds the main actor, so one
scenario's deferred publication lands in the next. The
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

`desktop_performance.py write-visible` opens one window on a private Home
through the real `lf monitor work --watch` reader, then commits from
`sqlite3`: a Task created and renamed, a Session created, renamed and completed.
Each interval runs from the writer's exit to the row read back from a captured
bitmap; `write_ms` is recorded beside it. The Home must be a copy (it refuses
the one in use) already at the schema of the `--lf` being measured, and a
current Task in `--repo` is the model for the new one. Written Tasks are
deleted after each attempt; completed benchmark Sessions stay in the copy. It
needs a logged-in desktop. Each write waits for an idle reader, so it does not
sample a commit landing behind a reading already in flight.

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

Add `--mounted` to check the production shell contract and 21 native Task-link
reopens. The reopen regression refreshes Session observations before each client
exits, then selects it again with the cached observation still present. It uses
the same transcript, input, companion pane, draft, selection and identity checks
as the combined soak, with its unchanged five-second endpoint. Native reopening
waits for selected Task, focus and native history before bitmap/OCR capture, then
rechecks readiness and the same deadline. This keeps pending-screen OCR from
blocking the main actor needed by connect and mounting; capture and input proof
remain required. Monotonic `uptime_ns` and transition-only `reopen_state` records
separate application readiness from observer cost. Comparisons must use the same
observer on both sides. `reopen.jsonl`
records requests and observed states. This focused regression takes no Instruments
trace and does not supply a matched performance distribution or hour coverage.

One OS sandbox, inherited by the native runner and every child, permits writes
only beneath the output directory, local sockets there and a fixed executable
set including the owned stub. Snapshot-named existing Git checkouts and their
Git metadata receive read-only access for discovery and status; their parent
directories and external credential homes do not. Network, external signals and
copied providers remain denied. The runner fingerprints tracked content/modes,
untracked names, ignore files, checkout identities, aliases, configuration and
Git metadata before and after. Changed inputs reject comparison. Git Trace2
records each command's exit, errors and internal duration in `git-events.jsonl`;
failed or unfinished Git observations reject scoring despite an outer CLI success.
Missing/non-Git checkout observations remain explicit. Denied work is not a
performance improvement. Input manifests stay beside the private receipts.
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
A four-case PTY probe isolates the launch boundary: `/bin/cat` and
`/bin/bash --noprofile --norc -c 'exec /bin/cat'` both returned the input marker
and exited 0. Both `/usr/bin/login -flp jack /bin/cat` and the captured
login/Bash form failed before login started: `execvp` returned Operation not
permitted (sandbox-exec exit 71). All four used the same inherited policy,
with only an exact login executable allowance added to a private probe copy.
The repository policy remains unchanged. `/usr/bin/login` is root-owned setuid;
the observation establishes rejection of that executable, not an independently
proved kernel explanation for the rejection. Receipts: `/tmp/loo304-login-diagnosis-20261006/probe.json`.

Pinned Ghostty `4c838723173da757a16a2f3afd4c94f16732ef6a` has two relevant contracts:
[embedded surface commands](https://github.com/ghostty-org/ghostty/blob/4c838723173da757a16a2f3afd4c94f16732ef6a/src/apprt/embedded.zig#L519)
are always shell strings, and
[macOS execution](https://github.com/ghostty-org/ghostty/blob/4c838723173da757a16a2f3afd4c94f16732ef6a/src/termio/Exec.zig#L1416)
wraps both shell and direct modes in login. A `direct:` prefix in the surface
string would not select config direct mode; even config direct mode retains
login. Loopflow's command-block patch does not change this path.

The production repair now lives in
[GhosttyKitPatches](../../../swift/GhosttyKitPatches/README.md). The lf3 framework
passes pinned Zig launch/block tests and the contained mounted shell contract:
login startup files, inherited environment/cwd, PTY input, production companion
shell after command exit, terminal markers and surface retention. The published
SwiftPM dependency now selects lf3 after Infrastructure verified the full public
download. Local-path overrides remain temporary proof inputs.

```sh
uv run python scripts/desktop_performance.py verify-fixture --mounted --lf target/debug/lf --output /tmp/ghostty-launch-proof
```

The final proof is `/tmp/loo304-lf3-reviewed-proof-20261006/native.log` (five
headless containment/history checks and one mounted test with three launches).
The policy permits signals only within the inherited sandbox and the exact
`/usr/bin/tty` needed by the production shell bootstrap. An external-parent
signal probe remains denied, alongside credential/file/provider/network checks.
The first lf3 replay exposed the old blanket signal denial hanging Ghostty's
child teardown; `/tmp/loo304-lf3-mounted-sample.txt` retains the stack, and
`/tmp/loo304-local-lf3-mounted-20261006/` retains the failed run. Exact owned cat
children were stopped to release that attempt; no successful outcome is claimed.

Production registration alone did not repair every font. The retained bitmap in
`/tmp/loo304-outline-pixels-20261006/` shows a mounted Wave row with missing-glyph
boxes. Typography now selects bundled bold/serif PostScript faces explicitly;
`strong(size)` owns the bundled Lato bold face. CoreText trait probes also found
account-library font candidates outside containment; that access remains denied.
The toolbar now contains its accessibility children rather than replacing their
identifiers. The harness enables in-process accessibility, explicitly opens
Detailed Flow, and waits for the loaded history list to mount before revealing it.
Shared-checkout setup retains the selected pane and explicitly reselects its
starting Session. These changes preserve every measured endpoint and population.

`/tmp/loo304-journey-history-painted-20261006/` passes all 34 catalogue scenarios
once (8 Tasks/4 Sessions and 256 Tasks/128 Sessions), with native presses, retained
PTY replies and draft/selection/pane checks. This is a journey replay, not p95 or
soak acceptance. Both OCR recognizers now inspect the same bitmap without custom
word hints or fuzzy substitutions: fast reads serif zeroes, accurate reads small
monospaced IDs. Their combined verification overhead must match on both sides;
older capture timings are not causally comparable. Failure-only PNG capture adds
no retained bitmap to successful measurements. Missing native controls list
available identifiers, snapshot CLI warnings remain in `native.log`, and failed
read receipts retain their reason. The real read adapter also supports the
workspace's `activity` query; the contained proof verifies copied Work activity.

The original October 5 snapshot lacks `execs_unfinished` and remains unchanged at
SHA-256 `2d4ec12d2103f3200d3675ec419f97a6b06b782c697f63f0ab9d80c583901725`.
`prepare-snapshot` produced `/tmp/loo304-compatible-snapshot-20261006-02/` from
all 39 source tables (342 Tasks, 604 Sessions, 218,637 Execs). Its private manifest
proves every source row/identity/payload/counter matches; only the index differs.
No live Home or retained copy was migrated. The initial URI-open failure is kept
in `/tmp/loo304-prepare-snapshot.log`; the corrected preparation passed.

`/tmp/loo304-compatible-replay-20261006/` passes one cold/warm/reopen Task capture
(1,341/308/229 ms); native reopening fails. These single observations cannot
establish p95, usable combined-workspace acceptance or improvement. The preceding
schema-refusal replay remains `/tmp/loo304-snapshot-schema-replay-20261006/`.

The database population spans 36 repository paths. The initial policy denied
other repositories' `.lf/config.yaml` and Wave goals. A config-only diagnostic
restored all 45 current Waves without unavailable Task collections. The runner
then granted only those exact authored configuration files. The later
[Git correction](20261006-matched-rust/README.md#corrected-repository-observations--october-6)
adds bounded checkout/status observations and fingerprints their inputs.
Missing files stay missing; credentials, provider/network access, filesystem
mutations and host signaling remain denied.

The owned fixture now follows the repository's configured Team and uses its
repository for exact Task queries; native execution retains its owned checkout.
The configured-repository containment proof exercises both differences. Local
Task status, Flow/history and owned activity reads now use the existing CLI;
provider-backed comments remain unavailable and appear in report.md/report.json.
After these repairs `/tmp/loo304-detail-replay-20261006/` passes four journeys
once. Earlier compatible/configured/team replays remain failed evidence.

The first replay preserved every historical row and appended seven inspection
Execs to its disposable runtime copy. Fixture equivalence does not mean a copied
Session has local execution authority. Original snapshots and installed Homes
remain untouched.

The 21-sample/3,600-second candidate attempt at
`/tmp/loo304-candidate-soak-20261006/` failed on disk exhaustion after seven
preserved rounds. Source and CLI hashes remained stable. Partial observations:

| Scenario / signal | Successful n | Median | p95 / result |
| --- | ---: | ---: | ---: |
| Warm cohort, cold workspace construction | 20 | 1,024 ms | 1,048 ms |
| Warm Task capture | 20 | 276 ms | 282 ms |
| Reopen Task capture | 20 | 202 ms | 208 ms |
| Native reopen, warm cohort | 6 of 7 | 1,770 ms | insufficient n; then timeout |
| RSS growth after four rounds | — | — | 52.7 MiB (budget <32 MiB) |
| Process CPU, partial 240 seconds | 229 | 0.1% | 74.0% |

First interactions remain separate in the raw report. RSS rose from 256.6 to
369.4 MiB across the partial recording. CPU excludes children. There were 478/478
CLI starts/ends, 2,606 SQLite connections, 239,598 statements and 2,875,164 rows.
Thirteen planned observations never started. The bitmap/OCR endpoint includes
observer cost and proves neither cold application launch nor compositor latency.
The failed source receipt has no matched baseline or completed trace clocks;
it cannot establish either hour soak, idle hangs/hitches or an improvement.

An Instruments temporary `.ktrace` reached 78.6 GiB with timestamps matching the
recording; no exact path-ownership receipt survived. It remains intact, with no
open handle at inspection. The run's output is 1.3 GiB and the filesystem has
about 10 GiB free. All failed probes and raw data remain private on this host.
A later permission error also obscured the outer receipt's native metadata;
the runner now keeps that metadata as it proceeds, including across later
exceptions. The original failed receipt was not rewritten.

Do not retry the full recorder on the same capacity. A capable isolated storage
path, or a verified lower-volume capture retaining the same full interval and
clocks, is the next prerequisite. Shortening the soak or retaining only a final
time window would not satisfy it. The prepared `501073137` baseline source
archive has identical CLI instrumentation and fixture schema; current Swift
would stay constant, so that pair isolates Rust changes only. Compilation and
baseline recording were not started after that capacity failure. The later
[matched Rust pair](20261006-matched-rust/README.md) completes twenty warm
observations per scenario with short RSS diagnostics; full hours remain open.

A matched realistic pair (21 samples and 3,600 seconds each), all original
budgets, and complete journey coverage remain required. Neither these focused
launch proofs nor the partial replay establish p95 or soak acceptance. No remote
performance host/job is configured; `ci.yml` headless/compile checks do not supply
that evidence. Jack Heart authorized source publication and verified GhosttyKit artifact updates;
merge and Task completion remain unauthorized on partial measurements.

Source gate on October 6: the affected runner passed 2,250 Rust tests (20 skipped),
370 Python, 78 website (three skipped), 377 headless Swift tests, formatting,
Clippy and architecture/boundary checks. The separate standard Xcode
`build-for-testing` passed. The rebuilt source CLI passed all four contained
proofs at `/tmp/loo304-final-source-contained-20261006/`. Gate and Xcode logs:
`/tmp/loo304-gate-20261006.log` and `/tmp/loo304-xcode-fallback-20261006.log`.
These checks establish source behavior, not rendered responsiveness or installation.

### Recorder capacity diagnosis (October 6)

The recorder now preserves `trace-recording.json` and `xctrace.log`, requests a
private `trace-tmp` directory, and records raw `.ktrace` paths observed open by
its own child. Instruments can ignore TMPDIR: the final two-second diagnostic
confirmed an account-temporary path through that child's open descriptor.
The original 78.6 GiB trace still has only timestamp attribution and is untouched.

Recording requires 8 GiB initial free space. It stops its own xctrace child after
2 GiB volume consumption, below 6 GiB free, or after the requested duration plus
30 seconds for shutdown. Storage is sampled, so these are stop thresholds rather
than filesystem quotas; unrelated volume writes can also stop a recording.
Failed captures and exact-path receipts remain intact. Recorder failure exits
nonzero and cannot complete the combined journey. No tail window substitutes
for full soak coverage.

A smaller diagnostic template is available through the existing `--template`
option plus repeated `--instrument` options: Xcode's full path to
`Packages/Base.instrdst/Contents/Templates/Blank.tracetemplate`, with instruments
`Hitches` and `Hangs`. The template's bare name is not discoverable. A 30-second
owned-sleep probe retained both table schemas and dated trace clocks, but also
798,774,536 bytes of raw trace (about 94 GiB/hour by linear extrapolation), while
its final bundle was only 21 MiB. This does **not** establish Desktop metric
quality or a sustainable hour recorder; the default template remains unchanged.
Receipts: `/tmp/loo304-minimal-recorder-30s-path-20261006/`; final bounded
record/export/summarize proof: `/tmp/loo304-bounded-recorder-final-20261006/`.
Both earlier failed template-selection probes remain retained.

Existing candidate evidence separates observer cost: all 21 retained observations
have median reopen bitmap capture 29.5 ms versus 201.7 ms through OCR; warm Task
capture 104.5 ms versus 276.4 ms through OCR. These include the first observation,
so they are diagnostic medians, not the warm-only acceptance cohort. No endpoint
or budget changed. Sampled RSS ended the first idle phase at 307.1 MiB, fourth
at 308.6 MiB and seventh at 313.5 MiB, while the first sampler reading was
256.6 MiB. This motivates separating initial/native/OCR allocation from retained
app growth; it neither diagnoses a leak nor removes the measured fourth-round
52.7 MiB failure. A capable recorder/storage environment and production allocation
profiling remain necessary before a matched pair and both full hour soaks.

### Observer allocations (October 6)

`observe_pixels.swift` isolates the two existing OCR passes against one retained
bitmap without creating a window, workspace, terminal or provider. Compile with
`xcrun swiftc scripts/benchmarks/desktop-performance/observe_pixels.swift -o OUTPUT`;
invoke `OUTPUT IMAGE pooled|unpooled` inside a read-only sandbox permitting that
exact image and executable, system frameworks, and only owned output writes.
Keep network and other executable access denied. The retained policy and commands
are `/tmp/loo304-observer-allocation-20261006/{observer.sb,runs.json}`.
Each process performs at most 40 observations and stops unsuccessfully above
512 MiB RSS. This is a fixed-pixel diagnostic, never a Desktop acceptance runner.

Four serial processes used the identical 2800×1600 retained failure bitmap,
identical executable and policy, with reversed pooling order. All 160 observations
completed with the same recognized-text hash. Private pixels/text stay local;
[raw measurements](20261006-observer/) contain only counts and hashes.

| Mode, invocation order | Initial / first OCR / final / drained RSS (MiB) | Warm OCR median (39 samples) |
| --- | --- | --- |
| Unpooled, 1 | 12.1 / 111.6 / 134.7 / 117.6 | 191.3 ms |
| Pooled, 2 | 12.0 / 98.2 / 127.4 / 110.3 | 191.5 ms |
| Pooled, 3 | 12.0 / 96.4 / 127.2 / 127.2 | 189.2 ms |
| Unpooled, 4 | 12.0 / 91.4 / 117.2 / 117.2 | 194.1 ms |

No monotonic pooling benefit supports changing the observer or declaring a leak.
The observer alone explains substantial startup allocation and latency. It cannot
explain the soak's fourth-round increase: OCR had already run before that interval.
A separate, intrusive `vmmap -summary` observation of the exact owned probe found
9.2 MiB allocated in the default malloc zone versus 45.4 MiB resident/dirty there
(36.2 MiB slack). Framework/shared mappings and non-malloc memory are separate;
neither zone slack nor total mapped resident pages measure retained app objects.
That probe is excluded from the four timing cohorts. Its raw output, preliminary
probes and first policy's exit-134 failure remain in the same private directory.

Journey journals now retain process RSS at event boundaries and per-capture RSS
before capture, after bitmap creation and after both OCR passes, with separate
bitmap/OCR durations. Failed captures preserve partial checkpoints. They retain
the original action-to-capture and action-to-verification clocks and assertions;
use identical instrumentation for both variants. These are process observations,
not allocation ownership: other threads can allocate between checkpoints.

The fixed-image diagnostic did not establish rendering availability. The contained
workspace replay below supersedes the earlier unsupported claim that this host
had no rendering environment. The failed 52.7 MiB result, twenty-sample acceptance
cohorts, unavailable provider comments and both 3600-second soaks remain outstanding.

### Contained workspace memory phases, October 6

```sh
uv run python scripts/desktop_performance.py run --output /tmp/workspace-memory --samples 5 --soak-seconds 150 --no-xctrace --cli target/debug/lf --snapshot /tmp/loo304-compatible-snapshot-20261006-02 --repo /Users/jack/src/loopflow --issue LOO-368
```

`--no-xctrace` passes through the recorder's RSS/signpost mode. It leaves trace
clocks, idle hitches and hangs unmeasured; it cannot establish hour acceptance.
Comparisons require matching recorder modes. Four-round growth now uses the
in-process `soak_begin` and fourth `soak_round` RSS checkpoints. The previous
asynchronous recorder baseline could miss early allocations and sample after the
round boundary. Missing checkpoint bytes stay unmeasured, including old journals;
original receipts remain unchanged. Setup, native input, navigation/files and
surface-release events also retain RSS without changing actions or endpoints.

`/tmp/loo304-memory-phase-replay-20261006/` passed all 20 endpoints (five each).
Warm `duration_ms` medians, four observations each, were 1,153 ms cold-workspace
construction, 316 ms warm Task, 269 ms reopen and 1,073 ms native reopening.
No p95 is available. Native capture median was 79.6 ms plus 237.0 ms OCR per
capture; both recognizers and all identity checks remain. Observer costs are
reported separately, never subtracted from the action duration. Each native
sample retained one visible window and terminal focus; key-window `-1` remains
an endpoint limitation. Scenario CLI volume: 553 connections, 45,987 statements,
539,392 rows.

`/tmp/loo304-memory-phase-soak-20261006/` passed five preserved rounds over 150 s
with 150 CPU/RSS samples, 246 complete CLI processes, 1,343 connections, 111,765
statements and 1,356,241 rows. App CPU median/max was 1.5/78.4 percent, excluding
children. Exact RSS began at 224.7 MiB; round ends were 280.1, 285.7, 277.4,
277.3 and 277.5 MiB. Fourth-round growth is **52.6 MiB**, failing <32 MiB.
Its original asynchronously sampled report says 52.1 MiB and is preserved.
No startup cost is warmed away or removed from the budget.

This run includes intrusive `vmmap -summary` at the exact journal PID and one
`leaks` inspection, so it is diagnosis, not an acceptance comparison. Default
malloc allocated bytes were 35.6 MiB after the first round and 35.4 MiB after the
fourth; resident zone pages were 87.1 and 93.7 MiB. Empty large allocations and
IOSurface mappings remain visible in the raw maps. These distinguish allocator
residency from live heap size but cannot attribute first-launch growth to an app
allocation stack. `leaks` exited 1, warning that the signed test helper was not
debuggable and only readonly memory was accessible; its 5,376-byte report is not
proof of a full leak scan. No security permission or containment was broadened.

The largest increase occurs within the first native input/capture phase; later
rounds do not show monotonic retention. This neither proves nor excludes an app
leak. A supported production repair still needs first-native-phase allocation
attribution across Ghostty/Metal, bitmap capture and framework caches. The new
checkpoints and exact-process maps narrow that work; they do not justify cache
purging, autorelease changes or budget revision. All private raw evidence stays
in the named directories. No copied identity, source snapshot or live Home changed.

`/tmp/loo304-memory-phase-confirm-20261006/` verifies the final reporting code
without intrusive tools: 20/20 endpoints, five preserved rounds, 150 seconds and
150 resource samples; source, CLI and authored configs remained unchanged. RSS
began at 222.0 MiB and ended round four at 276.6 MiB: **54.5 MiB growth**, still
failing. CPU median/max was 1.5/81.6 percent. All 250 CLI processes ended, with
1,363 connections, 112,753 statements and 1,368,111 rows. No timeouts occurred;
refused provider comments and cancellation read receipts remain visible. This
confirms the failure without treating diagnostic durations as hour coverage or
four warm observations as a scored p95. Focused runner/recorder tests pass 54;
Ruff and diff checks pass. No production memory fix, merge or completion is claimed.

### First-native allocation isolation, October 6

`terminalMemoryPhases` is an opt-in contained diagnostic (`LOOPFLOW_MEMORY_PHASES=1`,
`LF_DESKTOP_PERF_OUTPUT`, and an owned `LOOPFLOW_TEST_NATIVE_FIXTURE`). Launch it
through the runner's `_native_command`, as for `embeddedLaunchContract`; it uses
only an owned cat PTY. It pauses three seconds at each journal PID/phase so an
external `vmmap -summary` can inspect that exact process. The retained invocation
is `/tmp/loo304-phase-probe.py`; receipts are
`/tmp/loo304-terminal-phases-20261006/`. This diagnostic deliberately retains one
bitmap across its bitmap/OCR phases; its final RSS is not a leak measurement.

| Standalone phase | Immediate RSS MiB | Observation |
| --- | ---: | --- |
| Blank window | 83.3 | No terminal or workspace |
| Blank capture/OCR | 145.0 | Blank pixels do not establish warmed text recognition |
| Terminal rendered, input returned | 134.3 | No terminal capture yet |
| Terminal bitmap | 211.0 | 2800 × 1600, 11,200 bytes/row; 17.1 MiB bitmap |
| Terminal OCR | 239.5 | Both unchanged recognizers |
| Recapture/OCR | 307.1 | First bitmap still deliberately retained |

The rendered→bitmap maps add 17.1 MiB of CG image storage, about 17 MiB of
IOSurface storage and 17.2 MiB of empty large malloc allocations. Default-zone
live bytes stay about 14 MiB. These are region deltas, not allocation stacks;
map acquisition follows the immediate RSS checkpoint. Pinned Ghostty allocates
three size-dependent IOSurface render targets and grows initially tiny atlases.
Neither the maps nor source inspection establishes an oversized atlas or an
unnecessary production render target.

A controlled full-workspace variant delays bitmap/OCR until after first native
input. In `/tmp/loo304-workspace-capture-isolation-retry-20261006/`, first-phase
RSS is 225.5 MiB before launch, 241.0 MiB after input without capture, and
322.1 MiB immediately after capture/OCR. The capture map adds only 0.8 MiB of
live default-zone allocations (35.7→36.5), 25.1 MiB of empty large allocations
and 0.3 MiB of resident IOSurfaces. Thus much of this first-phase increase can
occur in capture/recognition without comparable live workspace retention.
This does not attribute every retained byte or justify subtracting observer cost.

The variant changes observation ordering and is **not acceptance**. Its second
reopen timed out; the failure capture and original records remain. The first
attempt at `/tmp/loo304-workspace-capture-isolation-20261006/` trapped after
using a prior capture timestamp in the no-capture branch (exit -5); the corrected
diagnostic leaves capture timing null. Neither change enters the ordinary runner.

Two proposed repairs were tested on the original five-sample/150-second workflow:

| Experiment | Endpoints | Round-four growth | Disposition |
| --- | ---: | ---: | --- |
| Reuse a size/scale-matched bitmap | 20/20 | +48.4 MiB | Rejected; still fails <32 MiB |
| Autorelease scope around full capture/OCR | 20/20 | +53.0 MiB | Rejected; no demonstrated benefit |

Receipts and exact experimental source are under `/tmp/loo304-bitmap-reuse-20261006/`
and `/tmp/loo304-capture-pool-20261006/`. The reconstructed reuse source matches
its recorded aggregate source hash. The scopes preserve all pixels, recognizers,
identities, populations and endpoints; neither changes the baseline or prewarms
startup. Both experiments were removed. No production allocation repair is
supported by these results.

Remaining options are a controlled capture implementation comparison with exact
pixel/recognition equivalence, or allocation attribution inside AppKit/Vision's
first terminal capture. The latter need not require the restricted `leaks` tool.
A renderer patch needs evidence of avoidable renderer allocation first. The
original <32 MiB budget, matched twenty-successful-sample p95s, both full hours,
and hitch/hang/compositor proof remain outstanding; none of these short probes
changes those obligations. No trace, snapshot, live Home or external authority
was changed.

Final unchanged-workflow replay: `/tmp/loo304-firstphase-final-replay-20261006/`
passed 20/20 endpoints and five preserved rounds over 150 seconds. Round-four
growth was **54.5 MiB**, still failing. Warm medians (four observations each) were
1,224 ms cold-workspace construction, 340 ms warm Task, 280 ms Task reopen and
1,318 ms native reopen; p95 remains unavailable. All 246 CLI processes ended,
with 1,343 connections, 111,761 statements and 1,356,237 rows. The source hash
stayed identical; unavailable comments/cancellations remain visible. This replay
restores every original capture and shows that the diagnostic-only reorder was
not carried into the shipped measurement path.
