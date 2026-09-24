# Performance

```bash
uv run python scripts/desktop_performance.py run --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --output /tmp/desktop-after \
  --baseline /tmp/desktop-baseline
```

Run the two desktop journeys on a macOS host after changing the outline or Task
workspace. `--samples 1` runs the short behavioral check; the default records one
first interaction and twenty warm attempts per scenario and population. The
runner opens an owned native window and three retained `/bin/cat` PTYs, with
fixed populations of 8 Tasks/4 Sessions and 256 Tasks/128 Sessions. Planning and
active Runs come from synthetic shared DTOs; no configured Home or provider is
used.

`hierarchy_interaction_ms` covers full/compact/Session presentations, folding,
expansion and filtering. `task_workspace_ready_ms` covers active/empty Monitor,
retained Session return and combined-pane zoom/restore. The endpoint is native
bitmap capture with text verification; Session return additionally requires
actual first responder, retained surfaces, draft submission and PTY replies.
Forced capture and OCR add observer overhead. Each attempt separates the last
successful capture time from its text-verification cost; total duration includes
verification and any input proof. These are **not compositor paint measurements**.
Frame hitches, scrolling during refresh,
production phase attribution and configured registry/provider costs remain
unmeasured. No rendering budget is scored from this endpoint.

Each output directory retains `attempts.jsonl`, `native.log`, `run.json` and
JSON/Markdown reports. Begin records preserve interrupted attempts. Failure rates
include failed, timed-out and interrupted attempts; planned observations that
never started are counted separately. Missing, repeated or mismatched observations
and source drift mark the run incomplete. Comparisons require matching host,
population, endpoint and build mode and unchanged measurement/fixture source.
p95 needs twenty successful samples in the same scenario/state.
Recover a report after interruption with:

```bash
uv run python scripts/desktop_performance.py report /tmp/desktop-baseline
```

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
