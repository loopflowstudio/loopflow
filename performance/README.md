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
active Sessions come from synthetic shared DTOs; no configured Home or provider is
used.

`hierarchy_interaction_ms` covers full/compact/Session presentations, folding,
expansion, filtering and scrolling during planning refresh. The scroll case uses
a 300-point viewport for both populations. A held fixture response keeps the shared
planning reader in flight while the native list scrolls to its final Task. Captured
text verifies that destination before releasing the response; refreshed text must
then appear without changing the settled viewport, selection or Session identities.
The normal 800-point viewport is restored before the workspace scenarios.

`task_workspace_ready_ms` covers active/empty Monitor,
retained Session return and combined-pane zoom/restore. The endpoint is native
bitmap capture with text verification; Session return additionally requires
actual first responder, retained surfaces, draft submission and PTY replies.
Forced capture and OCR add observer overhead. Each attempt separates the last
successful capture time from its text-verification cost; total duration includes
verification and any input proof. Scrolling includes an intermediate capture/OCR
before refresh release; its verification cost is also recorded in the attempt's
observation, alongside before/after labels and offsets. These are **not compositor
paint measurements**. Scrolling uses the native scroll API, excluding wheel-event
delivery and continuous gesture smoothness. Frame hitches, production phase
attribution and configured registry/provider costs remain
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

The flow runs the Home audit, renders the deterministic lifecycle
scorecard, then publishes the weekly [context cost](context-cost.md) readings. The generator is an internal operation so it stays available to
scheduled telemetry without becoming a general-user command or stable DTO.
The lifecycle scorecard reports retained evidence without publishing the retired
`task-loop-trust` metric. Context cost publishes its current metric contracts.

`lf usage --binds` compares Task and Wave totals under the two
[bind attribution](bind-attribution.md) rules.

`budgets.json` is the policy source. Each scorecard row carries its budget,
measured/eligible coverage, nearest-rank p50 and p95, verdict, and the exact
reason evidence is incomplete. `FAIL` outranks missing coverage when an
observed value already breaches a budget. `PASS` requires complete coverage
and at least 20 samples; smaller complete sets are `COLLECTING`.

The Rust telemetry operation supplies the current `SessionHistory` projection from
its selected Home. Python reads that projection, Task PR owner facts, and gate
receipts. It does not query retired SQL Runs/Turns or reduce provider streams.

| Row | Eligible fact | Measured value |
|---|---|---|
| `session_elapsed_seconds` | Recorded Session input ending inside the window | Recorded end minus observation, including inputs that began before the window |
| `session_total_input_tokens`, `session_output_tokens`, `session_cost_usd` | Same settled Session input | Direct provider usage with final receipts and no recorded gaps |
| `task_pr_to_merge_seconds` | Requested-and-merged Task PR in the window | GitHub merge time minus Task PR creation |
| `publication_to_merge_seconds` | Same Task PR | GitHub merge time minus publication request |
| `land_to_merge_seconds` | Same Task PR | GitHub merge time minus merge request |
| `recorded_attempt_to_merge_seconds` | Same Task PR | GitHub merge time minus earliest retained, explicitly attributed managed provider attempt |
| `avoidable_repairs`, `manual_git_repairs` | Same Task PR | Typed incident is `1`; tracked absence is `0` |

Usage values cover each retained Session input and its provider history. Their budgets start unset because Turn
budgets do not apply to this unit. Complete samples without a configured budget
are `UNBUDGETED`, not a performance pass. Missing provider cost is never priced
or replaced with zero; unfinished inputs are outside this settled-input cohort.

Task PR intervals require per-PR tracking and accepted GitHub merge evidence.
Missing/conflicting merge times remain unmeasured; when the time is missing,
`updated_at` includes the settled record in the coverage denominator only, never
as a substitute endpoint. Repository ownership comes from the Task's Wave, so
removing its worktree does not discard its history. These rows cover locally
recorded requested Task PRs, not standalone PRs or every GitHub diff.

“Recorded agent attempt → merge” uses the PR identity captured in managed Flow
membership. Attempts from earlier and unfinished Sessions contribute; unstarted
prepared Sessions do not. Historical or standalone Sessions without that identity
remain unknown rather than borrowing a Task's current PR. The first attempt
timestamp survives provider retries and must fall between PR creation and merge.
Token-usage gaps do not erase an independently recorded attempt.

This interval is an **observed lower bound**, not first-ever implementation start:
missing, pruned, uninstrumented or standalone work can hide an earlier attempt.
An attempt records the request to invoke the provider, not successful execution.
The row reports measured/eligible coverage and this limitation; it has no guessed
budget. Task PR creation → merge remains the durable lifecycle baseline.
First material progress and complete Task-loop intervals still lack current owner
facts: first-progress stays `UNKNOWN`, and the Task-loop trust instrument emits
`unavailable`. No Epoch-based history is recreated or inferred from Work state.

Coverage counts describe the selected recorded cohort. They do not prove the
completeness of all execution history. Compare equivalent units and cohorts
before attributing a change in duration to an optimization.

Generated reports are runtime evidence and stay out of source control. Examples
and fixtures must be synthetic; repository history owns only metric definitions,
budgets, schemas, and behavior tests.

For build/test time and submitted context by lifecycle step, use the local capture
collector and September 30 baseline in [check-cost.md](check-cost.md).
