# Desktop measurements: incomplete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 19/24; not started: 5.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 229.84 | — |
| small | compact | first_interaction | 1/1 | 212.40 | — |
| large | expand | first_interaction | 1/1 | 228.17 | — |
| small | expand | first_interaction | 1/1 | 211.59 | — |
| large | filter | first_interaction | 1/1 | 220.94 | — |
| small | filter | first_interaction | 1/1 | 208.92 | — |
| large | fold | first_interaction | 1/1 | 219.98 | — |
| small | fold | first_interaction | 1/1 | 209.08 | — |
| large | full | first_interaction | 1/1 | 232.09 | — |
| small | full | first_interaction | 1/1 | 216.36 | — |
| large | scroll_refresh | first_interaction | 0/1 | — | — |
| small | scroll_refresh | first_interaction | 1/1 | 195.73 | — |
| large | sessions | first_interaction | 1/1 | 259.14 | — |
| small | sessions | first_interaction | 1/1 | 209.45 | — |
| small | combined_restore | first_interaction | 1/1 | 263.03 | — |
| small | combined_zoom | first_interaction | 1/1 | 72.37 | — |
| small | monitor_active | first_interaction | 1/1 | 463.91 | — |
| small | monitor_empty | first_interaction | 1/1 | 78.09 | — |
| small | session_return | first_interaction | 1/1 | 256.63 | — |

See run.json, attempts.jsonl and native.log for failed, interrupted or unavailable observations.
