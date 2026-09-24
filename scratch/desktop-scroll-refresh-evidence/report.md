# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 504/504; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 248.11 | — |
| large | compact | warm | 20/20 | 260.28 | 287.92 |
| small | compact | first_interaction | 1/1 | 231.49 | — |
| small | compact | warm | 20/20 | 235.72 | 326.47 |
| large | expand | first_interaction | 1/1 | 237.33 | — |
| large | expand | warm | 20/20 | 263.55 | 303.40 |
| small | expand | first_interaction | 1/1 | 230.77 | — |
| small | expand | warm | 20/20 | 241.17 | 341.55 |
| large | filter | first_interaction | 1/1 | 229.79 | — |
| large | filter | warm | 20/20 | 253.44 | 296.51 |
| small | filter | first_interaction | 1/1 | 228.28 | — |
| small | filter | warm | 20/20 | 235.74 | 302.01 |
| large | fold | first_interaction | 1/1 | 225.71 | — |
| large | fold | warm | 20/20 | 252.97 | 301.02 |
| small | fold | first_interaction | 1/1 | 218.75 | — |
| small | fold | warm | 20/20 | 231.52 | 305.66 |
| large | full | first_interaction | 1/1 | 249.28 | — |
| large | full | warm | 20/20 | 264.09 | 359.24 |
| small | full | first_interaction | 1/1 | 215.94 | — |
| small | full | warm | 20/20 | 232.85 | 290.05 |
| large | scroll_refresh | first_interaction | 1/1 | 348.09 | — |
| large | scroll_refresh | warm | 20/20 | 357.22 | 647.27 |
| small | scroll_refresh | first_interaction | 1/1 | 319.25 | — |
| small | scroll_refresh | warm | 20/20 | 318.80 | 359.18 |
| large | sessions | first_interaction | 1/1 | 270.82 | — |
| large | sessions | warm | 20/20 | 280.96 | 312.43 |
| small | sessions | first_interaction | 1/1 | 228.54 | — |
| small | sessions | warm | 20/20 | 241.77 | 344.81 |
| large | combined_restore | first_interaction | 1/1 | 272.64 | — |
| large | combined_restore | warm | 20/20 | 307.22 | 350.24 |
| small | combined_restore | first_interaction | 1/1 | 298.82 | — |
| small | combined_restore | warm | 20/20 | 257.99 | 328.62 |
| large | combined_zoom | first_interaction | 1/1 | 60.17 | — |
| large | combined_zoom | warm | 20/20 | 90.22 | 99.01 |
| small | combined_zoom | first_interaction | 1/1 | 83.93 | — |
| small | combined_zoom | warm | 20/20 | 42.39 | 85.67 |
| large | monitor_active | first_interaction | 1/1 | 509.28 | — |
| large | monitor_active | warm | 20/20 | 246.28 | 268.83 |
| small | monitor_active | first_interaction | 1/1 | 518.28 | — |
| small | monitor_active | warm | 20/20 | 238.67 | 294.09 |
| large | monitor_empty | first_interaction | 1/1 | 113.00 | — |
| large | monitor_empty | warm | 20/20 | 110.79 | 117.87 |
| small | monitor_empty | first_interaction | 1/1 | 88.04 | — |
| small | monitor_empty | warm | 20/20 | 79.63 | 95.85 |
| large | session_return | first_interaction | 1/1 | 325.18 | — |
| large | session_return | warm | 20/20 | 326.23 | 357.28 |
| small | session_return | first_interaction | 1/1 | 290.42 | — |
| small | session_return | warm | 20/20 | 283.99 | 337.26 |

## Comparison

Different measurement_source
