# Live desktop recording — 2026-09-26T12:53:04

Process `Loopflow` (pid 25062), 90 s, macOS 26.0.1, Apple M4 Max. xctrace: recorded.
Signposts are the app's own `studio.loopflow`/`perf` intervals (end of SwiftUI commit; compositor presentation is up to one frame later). Hitch time is Apple's Animation Hitches table for this process: ≤5 ms/s good, 5–10 warning, >10 critical.

| Interval | Scenario | n | p50 ms | p95 ms | max ms | superseded |
|---|---|---:|---:|---:|---:|---:|
| lf | activity --since | 10 | 695.49 | 734.5 | 734.5 | 0 |
| lf | ls --all | 9 | 2004.25 | 2204.19 | 2204.19 | 0 |
| lf | ps --json | 61 | 273.83 | 293.18 | 314.27 | 0 |
| lf | roadmap --all | 9 | 3486.28 | 4020.7 | 4020.7 | 0 |
| lf | session list | 72 | 809.25 | 1450.33 | 1489.32 | 0 |

Hitches: 0 (0.0 ms/s, worst 0.0 ms); potential hangs: 1 (worst 1854.57 ms).
RSS: start 109.6 MiB, end 109.8 MiB, max 109.9 MiB, growth 0.1 MiB over 90 samples.
