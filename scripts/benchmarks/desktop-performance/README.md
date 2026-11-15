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
