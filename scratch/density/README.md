# LOO-298 density and behavioral proof

Jack Heart · 2026-09-30. Item 3 only. Production candidate
`6d76f74926dafef20aa86cf3c7247b8e5e3aab16`; no production changes in this item.
Main integration, the full local suite and the integrated gate remain with the
supervising session. No installed Home, provider or Desktop was exercised.

## Measurements

The ordinary debug CLI took about 300 ms even on an empty store. Bounded dense
list/detail reads stayed near that baseline. Their SQL and JSON encoding controls
were small; the broad Exec search miss was the exception, spending 64 ms in SQL.
Profile startup/schema validation before changing the bounded readers. This is
an inference from controls, not a sampled attribution of every CPU instruction.

Seed: one Wave, one Project, 1,000 Tasks, 20,000 AgentSessions, 5,000 FlowSessions,
100,000 Execs, 120,000 Session events and 5,000 Flow events. Sessions mix
conversation/Ask/review, headless/interactive and open/completed states; 2,500
Flows retain review membership. Each Session has a 4 KiB synthetic capture and
five 1 KiB output events. Each Flow has one captured skill with a 4,960-byte body.
Foreign keys passed. There are no live clients, external providers or accounts.
This stresses stored density, not concurrent writers or live process sampling.

The database began at 338,591,744 bytes and finished at 338,681,856 bytes. Actual
CLI reads also journal their own Exec: 100,043 rows after setup and 100,232 after
measurement. Thus the ordinary Exec inventory includes the benchmark's own
recent commands; the separate children page reads 100 of the seeded 99,999
children. Exact Exec detail reads a seeded completed command. The search-miss
control uses that same parent so it cannot match its own command text.

Milliseconds; list limit 100. Session list includes both interaction modes and
current reviews. Session detail is `session history` (six events); Flow detail
is `flow show --sessions` (one graph node), not an execution or provider resume.

| Read | CLI first / warm p50 / p95 | SQL first / warm p50 | JSON encode p50 | Output bytes |
| --- | ---: | ---: | ---: | ---: |
| Session page | 310.94 / 300.45 / 313.12 | 1.389 / 0.534 | 2.445 | 154,512 |
| Session history | 307.84 / 290.88 / 304.60 | 0.958 / 0.227 | 0.152 | 10,556 |
| Flow page | 297.41 / 295.54 / 301.29 | 1.143 / 0.294 | 0.600 | 37,811 |
| Flow detail | 293.48 / 304.51 / 546.63 | 0.875 / 0.194 | 0.013 | 738 |
| Exec page | 295.18 / 293.95 / 304.80 | 1.002 / 0.273 | 1.222 | 77,189 |
| Seeded Exec children | 296.48 / 307.75 / 325.61 | 0.989 / 0.290 | 0.864 | 54,069 |
| Exec detail | 295.95 / 312.53 / 323.32 | 0.918 / 0.151 | 0.008 | 466 |
| Session search miss | 309.00 / 301.24 / 322.33 | 1.714 / 0.932 | <0.001 | 3 |
| Seeded Exec search miss | 352.89 / 362.18 / 372.73 | 68.085 / 64.124 | 0.001 | 36 |

Startup controls: `lf --version` first **36.31 ms**, warm p50 **37.15 ms**,
p95 **38.57 ms**. Empty `session list --all --json --limit 100` first
**313.28 ms**, warm p50 **300.08 ms**, p95 **313.06 ms**. The Flow-detail p95
outlier remains in the samples; no cause was established.

Method and limits:

- CLI: 21 fresh processes per command, first reported separately and the next
  20 used for warm p50/p95. “Cold” means first command/fresh connection, **not**
  cold OS caches: seeding and earlier commands have already touched the disk and
  binary. No cache purge or privileged operation was performed.
- SQL: the helper traces the existing public command handlers, retaining only
  statements against the disposable database. It replays their exact expanded
  SELECT/WITH reads with the same bundled SQLite, preparing and stepping every
  returned column: first on a fresh connection, then 20 repeats on that connection.
  This includes store schema reads; it excludes in-memory schema reconstruction,
  PRAGMAs, writes, Rust DTO decoding and projection. It is a separate control,
  not an in-process timing subtracted from the CLI.
- Payload: 101 serializations of the returned JSON Value with the same
  `serde_json`, first excluded from warm percentiles. Pretty encoding is used
  consistently, including for the compact Session history response. This measures
  encoding cost, not DTO construction, history parsing, pipe transfer or rendering.
  The controls are **not additive** and their residual is not a precise startup
  breakdown. The empty command includes the store and runtime setup absent from
  the version control.
- Debug application code, optimized bundled SQLite as configured in Cargo,
  macOS 26.0.1 arm64; load averages 6.48 / 7.91 / 8.05 at start. No claim about
  release performance, the proposed 300 ms budget, installed data or Desktop
  latency follows. Comparison with the older four-store model was not run.

[Raw samples and binary digest](results.json) retain all timings and arguments.
Trace SQL and payloads are local under `.lf/tmp/density-final/`; the fixture Home
was deleted after completion. `.lf/tmp/test-compress/density-measure-final.log`
records the completed run. Earlier numbered attempts stopped on fixture/probe
issues: capture ID, path normalization, in-memory trace filtering, missing Task
placement and Exec self-observation. Their partial results are superseded.

## Production delta against main

Direct comparison of the candidate above with the available `origin/main`
`12013dae4fadc4946b885309ebcc22f1061e6493`, without fetching or integrating it:

| Production source | Added | Removed | Net |
| --- | ---: | ---: | ---: |
| Rust / Swift | 24,048 | 19,176 | +4,872 |
| Python / shell | 8 | 7 | +1 |
| SQL | 415 | 1 | +414 |
| Total | 24,471 | 19,184 | **+5,287** |

This repeats the branch's production-prefix estimate: test paths, docs and
scratch excluded; trailing inline Rust test modules excluded; no rename
detection. It counts physical lines including comments and whitespace, not
statements. It is an estimate rather than a Rust parser, and tip-to-tip comparison
also reflects main changes not yet integrated. Do not call gross removals a net
reduction. [Per-file counts and exact refs](code-delta.json) and
[the counting script](code_delta.py) preserve the method.

## Retained proof map

Each row names the maintained owner of a distinct behavior. The map reuses
existing proofs; this item adds no tests. Earlier compression already removed
retired inventories, duplicate reader matrices and decision-command trials.
The remaining unit, public-command and native-process cases test different
failure boundaries; no further equivalent duplicate was established.

Evidence paths below are under `.lf/tmp/test-compress/` unless prefixed otherwise.
“Fresh” means this item; retained receipts are not a final-tree gate.

| Behavior | Maintained proof | Evidence |
| --- | --- | --- |
| Name and conversation identity | `session_lifecycle_tests::conversation_keeps_its_name_and_identity_until_completed` | Fresh `density-behaviors.log` |
| Write-once bind, Started and prospective usage | `session_lifecycle_tests::binding_starts_the_task_once_without_reattributing_prior_work` | Fresh `density-behaviors.log` |
| Headless discovery | `session_lifecycle_tests::headless_history_is_discoverable_without_entering_the_interactive_list` | Fresh `density-behaviors.log` |
| Retry same conversation, then review | `session_lifecycle_tests::task_flow_retries_the_conversation_then_waits_for_review` | Fresh `density-behaviors.log`; replacement feedback also retained in `repairs.log` |
| Exact feedback and pass settlement | `session_lifecycle_tests::ask_returns_feedback_once_and_rejects_a_stale_answer`; `sqlite::flows::loop_positions_preserve_retry_and_settle_the_exact_pass_once` | Fresh `density-behaviors.log` |
| Typed advance/iterate/blocked and required evidence | `engine::flow_output::decisions_require_the_declared_value_and_evidence`; native `codex_connect.py --flow-blocked` | Fresh `density-boundaries.log`; retained `flow-blocked/results.json` records `blocked_driver_recovery=passed` |
| Successful completion excludes failed decisions | native `codex_connect.py --flow-decision-retry missing\|replace` | Retained `flow-decision-retry-{missing,replace}/results.json`; bounded correction also in `affected.log` |
| Driver transfer, provider fencing and Exec ancestry | `exec_ownership_tests::actual_engine_children_follow_driver_handoff_but_not_provider_replacement`; `retained_native_client_loses_writes_but_keeps_display_after_transfer` | Retained `native-ownership.log`, two passes; client replacement in `client-compress-native/results.json` |
| Exact child cancellation after driver/claim loss | `ops::task::task_stop_waits_for_selected_step_after_driver_death`; `task_worker_stop_does_not_mistake_a_released_claim_or_signal_for_exit` | Retained `cancellation-final-focused.log`; native engine-loss recovery in `cancellation-native-retry.log` |
| Chapter convergence across Homes | `ops::chapter::a_second_home_adopts_completed_rotation_through_planning_sync` | Fresh `density-behaviors.log`; independent mutation-loss and unknown-backlog races remain in retained `chapters.log` |
| Current review and Project-default migration | `migrations::ownership_cutover_keeps_current_task_review_without_importing_history`; `project_status_adoption_preserves_current_identity_and_custom_flow` | Fresh `density-behaviors.log`, populated prior-schema fixtures |
| Rust/Swift wire agreement | Rust `dto_fixtures` Session-history and Exec-page cases; Swift `DTOFixtureTests` | Fresh `density-behaviors.log` and `density-swift-dto.log` (18 Swift passes) |
| Retained terminal Task reads | `sqlite::flow_inventory::flow_inventory_distinguishes_task_selection_without_starting_done_work`; `ops::run::deleted_task_retains_context_and_run_attribution` | Fresh `density-boundaries.log`; public Exec discovery retains done-Task coverage in `affected.log` |

The native cases use real Codex against synthetic local Responses, not configured
provider acceptance. The cancellation receipt's Linux interposition remains
authored but unrun on this Mac. Earlier `affected.log` remains 44 passes/two
failures; `repairs.log` records those two repaired cases passing, not a rewritten
all-green original run. Swift's earlier broad discovery run had failures; the
fresh 18-case DTO pass does not establish the full Swift suite or rendered app.

This item's focused checks passed: 14 Rust cases across the two named logs,
18 Swift DTO cases, source CLI build, formatting, all-target Clippy (including
the temporary probe), Ruff and `git diff --check`. Clippy's first probe run
found a redundant integer cast; it was removed and the final run passed.

## Reproduce the measurement

From the repository root, build the ordinary candidate and the private probe:

```sh
cargo build -p loopflow --bin lf
mkdir -p rust/loopflow/examples
cp scratch/density/probe.rs rust/loopflow/examples/density_probe.rs
cargo build -p loopflow --example density_probe
uv run python scratch/density/measure.py
uv run python scratch/density/code_delta.py
rm rust/loopflow/examples/density_probe.rs
```

The runner scrubs inherited `LF_*`/`LOOPFLOW_*`, pins the source executable, Home
and database, creates only synthetic rows and removes its temporary Home on exit.
It checks page counts, exact detail and trace-handler output agreement. Exec
completion after the read is the one explicit output comparison exception.
The helper is temporary measurement code, not a shipped alternate reader.

Review found no reason to change production or add another benchmark dependency.
The important limit is preserved: separate controls locate the likely startup
cost but cannot quantify the in-process split. The next gate must use the merged
candidate; none ran in this item.
