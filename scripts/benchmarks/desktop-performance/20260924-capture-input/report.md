# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification; Session return also waits for owned PTY replies.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Fixture data excludes CLI/registry discovery, network and provider startup. Three retained cat PTYs per population.
Attempts: 462/462; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| large | compact | first_interaction | 1/1 | 248.06 | — |
| large | compact | warm | 20/20 | 263.51 | 370.76 |
| small | compact | first_interaction | 1/1 | 232.27 | — |
| small | compact | warm | 20/20 | 269.73 | 333.55 |
| large | expand | first_interaction | 1/1 | 253.89 | — |
| large | expand | warm | 20/20 | 263.22 | 371.99 |
| small | expand | first_interaction | 1/1 | 228.23 | — |
| small | expand | warm | 20/20 | 274.70 | 327.62 |
| large | filter | first_interaction | 1/1 | 238.77 | — |
| large | filter | warm | 20/20 | 254.84 | 357.01 |
| small | filter | first_interaction | 1/1 | 225.67 | — |
| small | filter | warm | 20/20 | 259.11 | 315.56 |
| large | fold | first_interaction | 1/1 | 241.87 | — |
| large | fold | warm | 20/20 | 255.18 | 356.56 |
| small | fold | first_interaction | 1/1 | 224.87 | — |
| small | fold | warm | 20/20 | 265.30 | 306.53 |
| large | full | first_interaction | 1/1 | 261.12 | — |
| large | full | warm | 20/20 | 261.57 | 377.92 |
| small | full | first_interaction | 1/1 | 231.61 | — |
| small | full | warm | 20/20 | 261.40 | 322.31 |
| large | sessions | first_interaction | 1/1 | 281.72 | — |
| large | sessions | warm | 20/20 | 282.71 | 416.12 |
| small | sessions | first_interaction | 1/1 | 233.40 | — |
| small | sessions | warm | 20/20 | 265.04 | 325.41 |
| large | combined_restore | first_interaction | 1/1 | 383.17 | — |
| large | combined_restore | warm | 20/20 | 303.46 | 419.83 |
| small | combined_restore | first_interaction | 1/1 | 288.71 | — |
| small | combined_restore | warm | 20/20 | 287.70 | 361.14 |
| large | combined_zoom | first_interaction | 1/1 | 85.49 | — |
| large | combined_zoom | warm | 20/20 | 87.25 | 130.96 |
| small | combined_zoom | first_interaction | 1/1 | 76.79 | — |
| small | combined_zoom | warm | 20/20 | 53.56 | 96.11 |
| large | monitor_active | first_interaction | 1/1 | 550.85 | — |
| large | monitor_active | warm | 20/20 | 363.40 | 859.76 |
| small | monitor_active | first_interaction | 1/1 | 493.40 | — |
| small | monitor_active | warm | 20/20 | 269.83 | 868.22 |
| large | monitor_empty | first_interaction | 1/1 | 124.98 | — |
| large | monitor_empty | warm | 20/20 | 106.96 | 157.10 |
| small | monitor_empty | first_interaction | 1/1 | 82.32 | — |
| small | monitor_empty | warm | 20/20 | 83.12 | 100.14 |
| large | session_return | first_interaction | 1/1 | 404.50 | — |
| large | session_return | warm | 20/20 | 326.44 | 495.64 |
| small | session_return | first_interaction | 1/1 | 267.08 | — |
| small | session_return | warm | 20/20 | 298.84 | 352.33 |

## Comparison

Different measurement_source
