# Desktop measurements: incomplete

Endpoint: in-process native bitmap capture with text verification.
**Not compositor paint time. Frame-hitch evidence is unavailable.**

Production Podium, router and local CLI reads over an isolated SQLite snapshot. Copied provider connections are refused. Cold workspace construction is not OS app launch. Session input readiness is unmeasured. Key-window and intermediate-sheet observations remain in each attempt.
Attempts: 9/9; not started: 0.
First interaction and warm samples are separate. p95 requires 20 successful samples. No budget is scored.

| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |
|---|---|---|---:|---:|---:|
| snapshot | cold_workspace | first_interaction | 1/1 | 13967.84 | — |
| snapshot | cold_workspace | warm | 2/2 | 20081.51 | — |
| snapshot | reopen_task | first_interaction | 0/1 | — | — |
| snapshot | reopen_task | warm | 1/2 | 7625.49 | — |
| snapshot | warm_task | first_interaction | 1/1 | 4815.04 | — |
| snapshot | warm_task | warm | 2/2 | 5919.68 | — |

See run.json, attempts.jsonl and native.log for failed, interrupted or unavailable observations.
