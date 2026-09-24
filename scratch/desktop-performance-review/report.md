# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 22/22; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 285.01 | — |
| small | compact | first_interaction | 1/1 | 229.96 | — |
| large | expand | first_interaction | 1/1 | 279.46 | — |
| small | expand | first_interaction | 1/1 | 227.23 | — |
| large | filter | first_interaction | 1/1 | 293.23 | — |
| small | filter | first_interaction | 1/1 | 232.17 | — |
| large | fold | first_interaction | 1/1 | 256.91 | — |
| small | fold | first_interaction | 1/1 | 212.63 | — |
| large | full | first_interaction | 1/1 | 284.15 | — |
| small | full | first_interaction | 1/1 | 212.47 | — |
| large | sessions | first_interaction | 1/1 | 314.87 | — |
| small | sessions | first_interaction | 1/1 | 228.64 | — |
| large | combined_restore | first_interaction | 1/1 | 308.01 | — |
| small | combined_restore | first_interaction | 1/1 | 295.33 | — |
| large | combined_zoom | first_interaction | 1/1 | 64.78 | — |
| small | combined_zoom | first_interaction | 1/1 | 79.60 | — |
| large | monitor_active | first_interaction | 1/1 | 973.37 | — |
| small | monitor_active | first_interaction | 1/1 | 784.88 | — |
| large | monitor_empty | first_interaction | 1/1 | 128.70 | — |
| small | monitor_empty | first_interaction | 1/1 | 83.99 | — |
| large | session_return | first_interaction | 1/1 | 362.99 | — |
| small | session_return | first_interaction | 1/1 | 279.97 | — |
