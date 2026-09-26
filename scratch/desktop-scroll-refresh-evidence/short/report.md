# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 24/24; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 225.26 | — |
| small | compact | first_interaction | 1/1 | 206.11 | — |
| large | expand | first_interaction | 1/1 | 221.03 | — |
| small | expand | first_interaction | 1/1 | 203.18 | — |
| large | filter | first_interaction | 1/1 | 214.92 | — |
| small | filter | first_interaction | 1/1 | 202.60 | — |
| large | fold | first_interaction | 1/1 | 215.46 | — |
| small | fold | first_interaction | 1/1 | 201.04 | — |
| large | full | first_interaction | 1/1 | 220.27 | — |
| small | full | first_interaction | 1/1 | 211.79 | — |
| large | scroll_refresh | first_interaction | 1/1 | 321.23 | — |
| small | scroll_refresh | first_interaction | 1/1 | 265.43 | — |
| large | sessions | first_interaction | 1/1 | 246.38 | — |
| small | sessions | first_interaction | 1/1 | 199.46 | — |
| large | combined_restore | first_interaction | 1/1 | 262.79 | — |
| small | combined_restore | first_interaction | 1/1 | 251.80 | — |
| large | combined_zoom | first_interaction | 1/1 | 57.13 | — |
| small | combined_zoom | first_interaction | 1/1 | 68.06 | — |
| large | monitor_active | first_interaction | 1/1 | 478.06 | — |
| small | monitor_active | first_interaction | 1/1 | 444.68 | — |
| large | monitor_empty | first_interaction | 1/1 | 104.56 | — |
| small | monitor_empty | first_interaction | 1/1 | 71.00 | — |
| large | session_return | first_interaction | 1/1 | 307.99 | — |
| small | session_return | first_interaction | 1/1 | 241.38 | — |
