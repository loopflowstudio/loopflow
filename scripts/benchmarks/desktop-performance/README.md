# Desktop performance

```sh
# Real usage on this machine: record the running app for a minute, then read the report.
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --process LoopflowMac   # dev build
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --no-xctrace           # signposts + RSS only
uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60 --template 'Time Profiler'   # attribute main-thread hangs; open hitches.trace in Instruments
uv run python scripts/benchmarks/desktop-performance/record_live.py summarize /tmp/loopflow-live-20260926-1200

# The older capture/OCR journeys and their hash-pinned baseline.
uv run python scripts/desktop_performance.py run --output /tmp/desktop-after --baseline scripts/benchmarks/desktop-performance/20260924-capture-input
```

`record_live.py record` attaches to the app you are already using (`Loopflow` from
/Applications, or `LoopflowMac` from `swift/.build`), waits `--seconds`, then writes
one directory: `signposts.ndjson` (the app's own `studio.loopflow`/`perf` intervals
from the unified log), `hitches.trace` plus exported `hitches.xml` and
`potential-hangs.xml` (Instruments' Animation Hitches template attached to the pid),
`rss.jsonl` (one `ps` sample per second), and `report.md`/`report.json` with
per-interval p50/p95/max, superseded counts, hitch ms/s, hangs and RSS growth.
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
