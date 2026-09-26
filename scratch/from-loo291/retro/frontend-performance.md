# Frontend performance: what to measure, how, and what we found

Jack asked for the core desktop experience to be optimized, starting from
measurement: cold start, Session/Wave/Task clicks, keystroke-to-glyph, memory
leaks — in dev/test and on his own machine, with no telemetry. This is the
research, the metric catalogue, the instrumentation that now ships in the app,
the harnesses, and the first numbers. Perf is a second pass by Jack's decision
(2026-09-26); this document records what the pass would consume.

## 1. Best practices (with sources)

**Signposts are the spine.** Apple built `os_signpost` to be near-free when
nothing records: argument formatting is compiled ahead, the work is deferred to
the Instruments backend ([WWDC18 405](https://developer.apple.com/videos/play/wwdc2018/405/)).
`OSSignposter` (macOS 12+) gives `beginInterval`/`endInterval`/`emitEvent`/`isEnabled`
([docs](https://developer.apple.com/documentation/os/ossignposter)). Verified on
this machine: `isEnabled` is true without Instruments, logd persists the
intervals, `log show --signpost --style ndjson` returns begin/end pairs with
`signpostID` and microsecond timestamps; `log stream` does *not* carry them
live; in-process `OSLogStore` returns none. So the lightweight recorder is "wait,
then `log show` the window".

**Hitches are the compositor-side truth.** A hitch is a frame shown later than
expected; the metric is hitch time in ms per second of wall time. Apple gives
two scales: WWDC20/XCTest/Instruments **<5 good, 5–10 noticeable, >10 distracting**
([WWDC20 10077](https://developer.apple.com/videos/play/wwdc2020/10077/),
[Tech Talk 10855](https://developer.apple.com/videos/play/tech-talks/10855)) and
the Organizer Hitches metric 10/25/50 ([Understanding hitches](https://developer.apple.com/documentation/xcode/understanding-hitches-in-your-app)).
This document uses the 5/10 scale because that is what Instruments reports.
Commit hitches (our SwiftUI body/layout missed VSYNC) are distinguished from
render hitches (render server missed). Hangs: <100 ms rarely noticed, ≥250 ms is
where Apple's tools start reporting ([Understanding hangs](https://developer.apple.com/documentation/xcode/understanding-hangs-in-your-app)).

**XCTest metrics.** `measure(metrics:options:)` runs `iterationCount+1` and
discards the first; `XCTOSSignpostMetric(subsystem:category:name:)` records the
elapsed time of a matching begin/end pair and nothing when there is none;
`XCTMemoryMetric` is physical footprint of the current process; `XCTClockMetric`
wall time ([XCTOSSignpostMetric](https://developer.apple.com/documentation/xctest/xctossignpostmetric),
[XCTMeasureOptions](https://developer.apple.com/documentation/xctest/xctmeasureoptions)).
XCTest prints every iteration's values under `swift test`, so a runner can take
p50/p95 without Xcode.

**Instruments from the CLI.** `xcrun xctrace record --template 'Animation Hitches' --attach <pid> --time-limit 60s --output x.trace`,
then `xctrace export --toc` to list tables and `--xpath '/trace-toc/run[@number="1"]/data/table[@schema="hitches"]'`
to dump one ([man page](https://keith.github.io/xcode-man-pages/xctrace.1.html)).
Verified here: attaches to the signed /Applications app without root; exported
schemas include `hitches`, `hitches-updates`, `potential-hangs`, `os-signpost`,
`display-vsyncs-interval`. Repeated cell values are written once with `id=` and
later as `ref=`; a parser must resolve them.

**Memory.** `leaks <pid>` on the notarized app says "Process is not debuggable"
and inspects read-only memory only (hardened runtime without
`get-task-allow`; [Hardened Runtime](https://developer.apple.com/documentation/security/hardened-runtime)).
Full leak graphs need the dev build (`swift/.build/debug/LoopflowMac`) or an
in-process loop with a footprint bound, which the harness now has. `heap`,
`vmmap`, `malloc_history` have the same restriction.

**MetricKit** (macOS 12+) is on-device but delivers daily aggregates to the
app and to Apple's Organizer for opted-in users ([MetricKit](https://developer.apple.com/documentation/MetricKit)).
It is a fleet telemetry channel by design and cannot be driven per run; it is
out of scope under "no telemetry".

**SwiftUI diagnostics.** `let _ = Self._printChanges()` explains why a body ran
(debug only, never ship; [WWDC23 10160](https://developer.apple.com/videos/play/wwdc2023/10160/));
the SwiftUI instrument (Instruments 26) shows Update Groups, Long View Body
Updates and a Cause & Effect graph ([WWDC25 306](https://developer.apple.com/videos/play/wwdc2025/306/)).
AttributeGraph cycles are os_log lines, proven absent by
`scripts/check_attributegraph_cycles.sh`.

**Launch.** Apple's target is first frame within 400 ms
([WWDC19 423](https://developer.apple.com/videos/play/wwdc2019/423/));
`DYLD_PRINT_STATISTICS` is gone since macOS 12, the App Launch template is the
pre-main path ([DTS](https://developer.apple.com/forums/thread/689581)). The app
now records the pre-main share itself from the kernel process start time.

**Perception and typing.** 100 ms is the limit for "instantaneous", 1 s for
uninterrupted flow ([Nielsen](https://www.nngroup.com/articles/response-times-3-important-limits/)).
For keystroke-to-glyph the editor/terminal literature treats single-digit ms as
excellent and ~50 ms as laggy: Terminal.app ≈6 ms p50, iTerm2 ≈44 ms
([Dan Luu, terminal latency](https://danluu.com/term-latency/)); GVim 0.9 ms,
Atom 49 ms ([Pavel Fatin, Typing with pleasure](https://pavelfatin.com/typing-with-pleasure/)).
The only reproducible methods are screen-capture ([Typometer](https://github.com/pavelfatin/typometer))
or a camera/light sensor; no in-process interval can see the photon.

**What the best desktop apps measure.** Zed works to an 8.33 ms frame budget at
120 Hz, diagnoses with the Metal HUD and Instruments, and found sub-4 ms frames
still stuttered because of presentation, not render time
([120 FPS](https://zed.dev/blog/120fps)). Ghostty separates IO and renderer
threads per surface, measures lock-held time, DOOM-fire FPS and `cat` throughput,
and Mitchell says input latency is "the one metric we have never once reliably
measured" ([discussion 4837](https://github.com/ghostty-org/ghostty/discussions/4837),
[PR 9662](https://github.com/ghostty-org/ghostty/pull/9662)). Warp benchmarks with
vtebench/Termbench and explicitly does not measure latency
([Warp](https://docs.warp.dev/terminal/comparisons/performance/)); Alacritty and
kitty point at Typometer ([Alacritty](https://github.com/alacritty/alacritty/blob/master/CONTRIBUTING.md),
[kitty](https://sw.kovidgoyal.net/kitty/performance/)). Linear's only first-party
material is the sync-engine talk, architectural, no numbers
([Linear](https://linear.app/now/scaling-the-linear-sync-engine)). Nobody in this
set publishes a keystroke-to-glyph method; the honest in-app proxy is
key-press → next draw, with Typometer as the external check.

## 2. Metric catalogue

All intervals are `os_signpost` under subsystem `studio.loopflow`, category
`perf` (`swift/LoopflowMac/Services/Perf.swift`). "Committed" = end of the SwiftUI
transaction, observed by `DispatchQueue.main.async` after the change; the
compositor presents ≤1 frame later. Budgets are p95 on Jack's M-series machine;
rationale follows each.

| Interval | scenario | Begins | Ends | Budget p95 | Why |
|---|---|---|---|---:|---|
| `cold_start` | launch | `LoopflowApp.init`; message has `pre_main_ms` | first non-empty outline committed | 1000 ms (400 ms to first frame) | Apple's 400 ms first-frame target; the rows need one `lf roadmap` + `lf session list`, so usable ≠ first frame |
| `hierarchy_interaction` | fold, presentation, filter, repository | button / binding setter | new outline rows committed | 50 ms | 50-Task Wave outline is one `LazyVStack`; under one 60 Hz frame budget ×3 |
| `task_workspace_ready` | wave | `PodiumModel.select` | Wave detail (objective, KRs, 50-Task plan) committed | 100 ms | Nielsen "instantaneous" |
| `task_workspace_ready` | task | `PodiumModel.select` | Task title, Flow diagram, status, Description committed | 100 ms | same; Comments and Runs must not be inside it |
| `task_workspace_ready` | session | `SessionsView.openSession` | Session pane became first responder | 100 ms retained / 500 ms new surface | retained switch is layout only; a new surface spawns a PTY |
| `terminal_key_to_draw` | — | `keyDown` in a Ghostty pane | next display-link draw | 16.7 ms (one frame) | libghostty renders on its own thread; the glyph follows the pty echo |
| `lf` | verb | subprocess spawn | stdout decoded | 300 ms; never on the main actor | it is a 47 MB binary per read; every 2 s today |
| `markdown_parse` | bytes | one Description/Comment parse | blocks built | 5 ms, and **never during a refresh that changed nothing** | it ran on every body evaluation |
| hitch time (xctrace) | — | — | — | ≤5 ms/s idle, ≤10 ms/s while scrolling | Apple's scale |
| footprint after 4 navigation rounds | — | — | — | growth < 32 MiB, flat after the first round | leak check, not a size budget |
| search-field keystroke | — | key event to `NSTextField` | `navigation.search` == typed and rows committed | 50 ms | same as filter |

## 3. How the recent product changes change what we measure

- **Wave → Task → Session drill-down** replaces the old Overview/Session mode
  switch. `task_workspace_ready` now has three scenarios with different ends: a
  Wave ends at the plan, a Task at Flow + status, a Session at *first responder*.
  Measuring "detail appeared" would undercount a Session by the surface attach.
- **Flow diagram** is inside the Task interval on purpose: it is laid out in the
  same body as the title (`FlowDiagram.layout(width:)` + a `Canvas`). If it ever
  moves to a `.task`, the interval must move with it or it becomes invisible.
- **Demand-loaded Comments and Runs** must not delay Task paint. The fixture
  delays the Comments read by 250 ms and the harness asserts the thread is still
  absent when the Task interval ends. The `lf pm task comments` read is its own
  `lf` interval, so a slow Linear read shows up as `lf` p95, never as Task p95.
- **Per-Task readings** (`TaskReadings`) mean the same Task selected twice must
  paint from cache while the read refreshes; the harness alternates Tasks and
  the recorder's `superseded` count exposes selections that were replaced before
  they finished.
- **Ghostty pane retention** means Session latency is only meaningful *across*
  navigation: the harness alternates two retained `/bin/cat` panes and asserts
  the surfaces are identical afterwards; the first open of a new surface is a
  different scenario with a different budget.
- **2 s polling** (`lf session list`, `lf activity`) and **15 s roadmap refresh**
  are the background load every foreground number sits on. Idle hitch rate is
  therefore a first-class metric: it is what the app does when Jack does nothing.

## 4. What is built

- `swift/LoopflowMac/Services/Perf.swift` — the signposter, the six interval
  names, `begin/end/endAfterCommit` with superseding, `measure` (sync/async),
  `isPending` (read by the harness), pre-main milliseconds via `sysctl`.
- Instrumentation at the boundaries: `LoopflowApp.init` (cold_start begin),
  `WorkspaceNavigator` (cold_start end, hierarchy begin in fold/filter/
  presentation/repository and end on row change), `PodiumModel.select` (wave/task
  begin), `WorkSurfaceView` (wave/task end after commit), `SessionsView.openSession`
  (session begin), `GhosttyMetalView.becomeFirstResponder` (session end),
  `GhosttyMetalView.keyDown`/`displayLinkFired` (key→draw), `RegistryQueryLocal`
  (every `lf` read), `MarkdownBlocks` (parse). No flags, no network.
- `swift/LoopflowTests/PerformanceCatalogueTests.swift` — XCTest harness
  (the package mixes XCTest and swift-testing; XCTest is required for
  `XCTOSSignpostMetric`/`XCTMemoryMetric`). Density fixture 4 Waves × 50 Tasks,
  20 started, 21 Sessions, Markdown on every Task, delayed Comments. Eight tests:
  cold start, Wave paint, Task paint before Comments, Session accepts input with
  retention, terminal keystroke→echo, outline filter keystroke, fold, navigation
  loop memory bound, 50-Task plan scroll hitches under a display link.
- `scripts/benchmarks/desktop-performance/record_live.py` (+ README, +
  `python/tests/test_record_live.py`) — records the running installed or dev app:
  `log show --signpost` window, `xctrace` Animation Hitches attached to the pid,
  `ps` RSS per second; summarizes p50/p95/max per interval and scenario,
  superseded counts, hitch ms/s, hangs, RSS growth, plus the legacy
  `sessions-latency` log metrics the installed build already emits.

## 5. Optimizations shipped in this pass (small, safe)

1. **Do not republish unchanged readings.** `PodiumModel` assigned `sessions`,
   `processActivity`, `waves` and `roadmap` on every poll; `@Observable` fires
   on assignment, so the whole workspace re-rendered every 2 s with identical
   data. Now `PodiumReading` is `Equatable` and each refresh assigns only on
   change. Evidence: the idle installed app hitched 10 times in 6 s (308 ms,
   51 ms/s, "Potentially expensive app update(s)") with nothing being clicked.
2. **Markdown parses once per source.** `MarkdownBlocks` is `Equatable` on
   `source` and both call sites use `.equatable()`, so a parent re-render for
   unrelated state no longer re-parses the Description or every comment body.
   `markdown_parse` intervals make the count visible in any recording.

## 6. Top 3 next optimizations (evidence-led)

1. **Stop the 2 s subprocess polls from touching the main path at all.** Every
   2 s the app spawns `lf session list` and `lf activity` (47 MB binary, one
   `Process` each) and decodes on a detached task, then publishes. With #5.1 the
   publish is now conditional, but the spawn/decode still runs and the roadmap
   refresh republishes a 200-Task snapshot every 15 s. Measure `lf` p95 per verb
   in a live recording; the fix is the retained `lf runs --active --watch` shape
   already used for active Runs (one stream, deltas), applied to Sessions.
2. **Outline rows are rebuilt from scratch on every navigator body.**
   `WorkspaceNavigator.rows` calls `model.workspace.outline(...)`, and
   `model.workspace` constructs a new `WorkspaceProjection` (grouping every
   Session under every Task) each access; `WorkSurfaceView` and `SessionsView`
   do the same for breadcrumbs and session lookups. Cache the projection on the
   model keyed by the roadmap/sessions identity it was built from; measure with
   `hierarchy_interaction` filter keystrokes at 50 Tasks per Wave.
3. **`FlowDiagram` lays out inside the body with a `Canvas` per row and
   `GeometryReader`-driven wrapping.** Selecting a Task pays layout for the full
   pinned graph before the title paints. If `task_workspace_ready scenario=task`
   p95 exceeds 100 ms at density, memoize `layout(width:)` per (graph, width) and
   keep node buttons stable across passes (`.id` on occurrence keys).

## 7. Receipts

Every command in this pass, with the log path. Scratchpad =
`/private/tmp/claude-501/-Users-jack-src-loopflow-main-view-task/f80b50d6-c50a-4489-8d81-d10a36f31ae3/scratchpad`.

| Step | Command | Result | Log |
|---|---|---|---|
| Resource preflight | `uv run python scripts/resource_envelope.py` | PASS; this checkout 11.5/12 GiB build budget | `resource-preflight.log` |
| Signpost probe | `swiftc probe.swift && ./probe; log show --last 20s --signpost --predicate 'subsystem == "studio.loopflow.probe"' --style ndjson` | `isEnabled=true`; 3 begin/end pairs returned by `log show`; `OSLogStore` in-process: 0; `log stream`: 0 | `probe.swift`, `stream-probe.ndjson` |
| Hitch attach probe | `xcrun xctrace record --template 'Animation Hitches' --attach 98115 --time-limit 6s` | exit 0, trace saved; **idle app: 10 hitches, 308 ms, 51 ms/s** | `probe-hitches.trace`, `xctrace-probe.log`, `probe-hitches.xml` |
| leaks probe | `leaks 98115` | exit 1: "not debuggable"; read-only scan: 6 leaks / 1344 bytes | `leaks-probe.log` |
| App build 1 | `swift build --package-path swift --product LoopflowMac --jobs 4 -Xswiftc -gnone` | FAIL: `MarkdownBlocks ==` crosses main-actor isolation | `build-app.log` |
| App build 2 | same, after `nonisolated static func ==` | PASS (exit 0, 10 s incremental) | `build-app2.log` |
| Python proof | `uv run pytest python/tests/test_record_live.py -q` | 1 passed (after the `ref=` parser fix: 1 passed) | terminal |
| Live recording (installed app, idle, pre-instrumentation build) | `uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 45 --output <scratch>/live-installed` | see §8 | `live-installed.log`, `live-installed/report.md` |
| Swift perf harness | `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter PerformanceCatalogueTests` | see §8 | `perf-tests.log` |

## 8. What was measured

**Real usage, installed app** (`/Applications/Loopflow.app`, pid 98115, up 13 h,
`--repo /Users/jack/src/loopflow`, idle — Jack was not using it; a foreign
`swift-frontend` from another Run was compiling at ~100 % of one core for the
whole window). This binary predates the instrumentation, so it has no
`studio.loopflow/perf` signposts; hitches, hangs and RSS are from xctrace/`ps`.

| Window | Hitches | Hitch time | Worst hitch | Potential hangs | Worst hang | RSS |
|---|---:|---:|---:|---:|---:|---|
| 6 s probe (`Animation Hitches`, no foreign build) | 10 | 308 ms = **51 ms/s** | 75 ms | — | — | — |
| 45 s recording (`record_live.py`) | 24 | 229 ms = **5.1 ms/s** | 25 ms | **8** (166–483 ms; four "Microhang" ≥ 416 ms) | 483 ms | 258.7 → 258.8 MiB, max 259.0 |

Reading: at idle the main thread stalls for 0.2–0.5 s eight times in 45 s, and
Instruments labels the hitches "Potentially expensive app update(s)" — a SwiftUI
commit, not the render server. The cadence (5.0, 12.8, 13.2, 16.5, 24.4, 25.4,
33.3, 35.2 s) is not one poll's period, so it is the sum of the 2 s Session/
activity polls and the 15 s roadmap refresh republishing readings on a real-size
workspace. RSS did not move. Nothing here is simulated; the foreign build is
noted because it inflates hitch counts (8 ms frames) but cannot create 400 ms
main-thread hangs in another process.

**Dev/test harness** (`swift test … --filter PerformanceCatalogueTests`, debug
build, density fixture, `/bin/cat` PTYs, same foreign build running):

| Test | Metric | Values (5 iterations after warm-up) | Result |
|---|---|---|---|
| `testColdStartToOutlineRows` | `cold_start` signpost (refresh → rows committed) | 100, 107, 104, 105, 105 ms — **avg 104 ms, rsd 2 %** | pass; budget 1000 ms |
| same | wall clock incl. model + window mount | 218–226 ms | — |
| same | peak physical footprint | 56 MB | — |
| `testNavigationLoopMemoryIsBounded` (run 1) | — | timed out on a navigation boundary | harness fix: `pump` now forces layout/display each poll like `DesktopPerformanceTests`; rerun below |

Rerun (`perf-tests.log`, fixed `pump`): `cold_start` 107–118 ms, **avg 112 ms,
rsd 4 %**, wall 227–248 ms (pass). `testNavigationLoopMemoryIsBounded` again
timed out, now at "navigation 0" — a plain `select(.wave(id: "wave-1"))`. Diagnosis:
the fixture answers `lf ls` with `[]`, so `model.visibleWaves` is empty and
`WorkSurfaceView.content` shows "No planned Work in this repository" instead of
`waveDetail`/`taskDetail`; the wave/task/session `task_workspace_ready` ends
therefore never fire in the harness (cold start passes because it ends in the
navigator). Fix, not yet applied: return the four Waves as `WaveSnapshot` rows
from `("ls", _)`, as `WorkspaceNavigationProofTests` does. The Session, typing,
fold, memory and scroll tests were not reached; the run was stopped to free the
CPU for the other Run. No harness number below cold start is established.

## 9. Files changed

Production (Swift):
- `swift/LoopflowMac/Services/Perf.swift` — new
- `swift/LoopflowMac/LoopflowApp.swift` — `cold_start` begin in `init`
- `swift/LoopflowMac/PodiumModel.swift` — `Perf.begin` in `select`; `PodiumReading: Equatable`; assign-on-change for sessions/processActivity/waves/roadmap
- `swift/LoopflowMac/Views/WorkSurfaceView.swift` — `onChange` end hooks on Wave/Task detail; `.equatable()` on `MarkdownBlocks`
- `swift/LoopflowMac/Views/WorkspaceNavigator.swift` — **also owned by the next Run**: two `onChange` modifiers on the outer `VStack` (cold_start / hierarchy end), `Perf.begin` in the fold button, the search `TextField` and presentation `Picker` bindings, and `selectRepository`
- `swift/LoopflowMac/Views/SessionsView.swift` — **also owned by the next Run**: one line, `Perf.begin(Perf.taskWorkspaceReady, "session", id: record.id)` after `model.select(subject)` in `openSession`
- `swift/LoopflowMac/Views/TaskCommentsView.swift` — `.equatable()` on `MarkdownBlocks`
- `swift/LoopflowMac/Views/MarkdownBlocks.swift` — `Perf.measure` around the parse; `Equatable` on `source`
- `swift/LoopflowMac/Services/RegistryQueryLocal.swift` — `Perf.measure(Perf.lf, verb)` around every read
- `swift/LoopflowMac/Services/Ghostty/GhosttyTerminalView.swift` — `import OSLog`; `keyToDraw` interval (`keyDown` → `displayLinkFired`); session end in `becomeFirstResponder`

Tests and tooling:
- `swift/LoopflowTests/PerformanceCatalogueTests.swift` — new
- `scripts/benchmarks/desktop-performance/record_live.py` — new
- `scripts/benchmarks/desktop-performance/README.md` — new
- `python/tests/test_record_live.py` — new
- `scratch/retro/frontend-performance.md` — this document

Not touched: `swift/Loopflow/Design/*`, `DesignSystem.swift`,
`swift/LoopflowMac/Design/WorkspaceStyle.swift`, `AppBootstrap.swift`,
`TESTING.md`, `performance/budgets.json`, the 20260924 baseline.

## What's next

Built: the `Perf` signposter and every boundary in §2; the XCTest catalogue
harness at the accepted density; `record_live.py` with README and a Python proof;
two safe optimizations (assign-on-change readings, Markdown `.equatable()`).

Measured: installed app idle hitch/hang/RSS (§8), fixture cold start 104 ms,
and whichever harness rows the rerun produced (§8). Not measured yet: the
instrumented build on Jack's real repository (needs the next promotion, or a
dev-build launch against `--repo`, which this pass did not do while a foreign
build and the harness were competing for CPU); keystroke-to-glyph with a camera
or Typometer (the in-app interval is key → next draw only).

Remaining, in order of value:
1. Record the instrumented build idle for 60 s against the real repo and
   compare hangs/hitches with the 45 s installed-app recording; then
   `--template 'Time Profiler'` on whichever still hangs to name the frame.
2. Land §6.1 (stream Sessions instead of polling) and §6.2 (cache the
   `WorkspaceProjection`), each with a before/after `record_live.py` pair and
   the harness rows for `hierarchy_interaction` and `task_workspace_ready`.
3. Put the §2 budgets into `performance/budgets.json` once twenty comparable
   samples exist per interval on the installed build, and let
   `scripts/desktop_performance.py`'s comparison rules apply to `report.json`.
4. Cold start on the real app: the `cold_start` message carries `pre_main_ms`;
   confirm the 400 ms first-frame target with the App Launch template on a
   fresh launch (this pass only measured the fixture's refresh→rows leg).
5. AttributeGraph: run `scripts/check_attributegraph_cycles.sh` on the
   instrumented build; the new `onChange` modifiers are the only new
   dependencies and should not cycle, but that is unproven.
