# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 24/24; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 240.37 | — |
| small | compact | first_interaction | 1/1 | 221.30 | — |
| large | expand | first_interaction | 1/1 | 237.41 | — |
| small | expand | first_interaction | 1/1 | 239.81 | — |
| large | filter | first_interaction | 1/1 | 229.24 | — |
| small | filter | first_interaction | 1/1 | 217.72 | — |
| large | fold | first_interaction | 1/1 | 226.09 | — |
| small | fold | first_interaction | 1/1 | 211.39 | — |
| large | full | first_interaction | 1/1 | 235.11 | — |
| small | full | first_interaction | 1/1 | 217.32 | — |
| large | scroll_refresh | first_interaction | 1/1 | 344.56 | — |
| small | scroll_refresh | first_interaction | 1/1 | 287.54 | — |
| large | sessions | first_interaction | 1/1 | 263.28 | — |
| small | sessions | first_interaction | 1/1 | 238.65 | — |
| large | combined_restore | first_interaction | 1/1 | 286.94 | — |
| small | combined_restore | first_interaction | 1/1 | 269.82 | — |
| large | combined_zoom | first_interaction | 1/1 | 65.99 | — |
| small | combined_zoom | first_interaction | 1/1 | 75.24 | — |
| large | monitor_active | first_interaction | 1/1 | 514.29 | — |
| small | monitor_active | first_interaction | 1/1 | 471.54 | — |
| large | monitor_empty | first_interaction | 1/1 | 111.58 | — |
| small | monitor_empty | first_interaction | 1/1 | 83.46 | — |
| large | session_return | first_interaction | 1/1 | 327.63 | — |
| small | session_return | first_interaction | 1/1 | 271.85 | — |
