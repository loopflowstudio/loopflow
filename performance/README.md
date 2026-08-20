# Performance

Run the repository's daily operator flow:

```bash
lf telemetry-daily
```

The flow runs the Home audit, then renders the deterministic lifecycle
scorecard. The generator is an internal operation so it stays available to
scheduled telemetry without becoming a general-user command or stable DTO.

`budgets.json` is the policy source. Each scorecard row carries its budget,
measured/eligible coverage, nearest-rank p50 and p95, verdict, and the exact
reason evidence is incomplete. `FAIL` outranks missing coverage when an
observed value already breaches a budget. `PASS` requires complete coverage
and at least 20 samples; smaller complete sets are `COLLECTING`.

Four lifecycle rows read durable owner facts rather than artifacts or observer
timestamps:

| Row | Eligible fact | Measured value |
|---|---|---|
| `task_first_progress_seconds` | Ended Task Run, windowed by `ended_at` | First material provider event minus Run start |
| `land_to_merge_seconds` | Explicitly requested Task PR, windowed by GitHub `merged_at` | GitHub `merged_at` minus first merge-request time |
| `avoidable_repairs` | Same requested-and-merged Task PR | `1` for a typed avoidable rebase-agent incident; tracked absence is `0` |
| `manual_git_repairs` | Same requested-and-merged Task PR | `1` for a typed raw-sequencer adoption incident; tracked absence is `0` |

Historical rows are never backfilled. Until a complete scorecard window lies
after the lifecycle-authority cutover, clean rows remain `UNKNOWN`; an observed
budget breach still reports `FAIL`. A missing or conflicting GitHub merge time
also remains unmeasured. Merge correctness does not depend on performance
evidence.

PRs still open at cutover begin merge tracking immediately, but not repair
tracking: their future GitHub merge boundary is coverable, while their earlier
repair history may already be incomplete.

Generated reports are runtime evidence and stay out of source control. Examples
and fixtures must be synthetic; repository history owns only metric definitions,
budgets, schemas, and behavior tests.
