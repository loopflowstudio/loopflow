# Desktop measurements: complete

Endpoint: in-process native bitmap capture with text verification.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Production Podium, router and local CLI reads over an isolated SQLite snapshot. Copied provider connections are refused. Cold workspace construction is not OS app launch. Session input readiness is unmeasured. Key-window and intermediate-sheet observations remain in each attempt.
Attempts: 15/15; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| snapshot | cold_workspace | first_interaction | 1/1 | 7873.29 | — |
| snapshot | cold_workspace | warm | 4/4 | 8878.11 | — |
| snapshot | reopen_task | first_interaction | 1/1 | 59.95 | — |
| snapshot | reopen_task | warm | 4/4 | 64.49 | — |
| snapshot | warm_task | first_interaction | 1/1 | 182.35 | — |
| snapshot | warm_task | warm | 4/4 | 167.59 | — |
