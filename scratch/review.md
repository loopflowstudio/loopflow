# Review: Obligation-aware ledger continuity

## Evidence Matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| August 4–11 are outside the cadence denominator | Explain each historical gap from durable activation and receipt evidence | The retained job identifies `infrastructure/telemetry-daily`, Home `home_39860354aaca640c2ccb50bf6ca609d8`, and schedule `0 0 9 * * *`; its file birth is 2026-08-22T17:23:51Z and its first exact scheduled receipt starts 2026-08-22T17:24:01Z | Live plist metadata and first retained receipt; `copied_production_history_does_not_block_the_telemetry_scorecard` | pass |
| A current missed run is actionable | Fail with the owning cron, Home, expected interval, and receipt-history action | Continuity matches scheduled receipts on wave, flow, target kind, Home, schedule, and interval, then prints every named field and the exact `lf cron history` command | `a_missing_due_receipt_names_the_cron_home_interval_and_action` | pass |
| Pre-activation history is not an outage | Do not create an obligation before activation | A schedule at or before the current time is due only when its interval starts at or after `activated_at` | `a_pre_activation_day_has_no_due_interval` | pass |
| Scheduler execution is distinct from target success | A scheduled failed target satisfies cadence; a manual run does not | Receipt source participates in the match and receipt outcome does not | `a_scheduled_failure_counts_but_a_manual_trigger_does_not` | pass |
| New evidence restores the current window without rewriting history | A later receipt makes continuity healthy while old gaps remain immutable diagnostics | Adding the exact scheduled receipt changes Fail to Ok; the event rows remain byte-for-byte equal and the gap detail remains visible | `a_later_receipt_restores_the_current_window_without_rewriting_history` | pass |
| Telemetry reaches its scorecard | Historical gaps alone cannot stop `telemetry-daily` before `__telemetry-scorecard` | The copied store preserves all August events, doctor reports all eight dates as pre-activation history, and the configured flow writes its scorecard marker | `cargo test -p loopflow --test doctor_tests copied_production_history_does_not_block_the_telemetry_scorecard -- --exact` | pass |
| Activation survives reconciliation | Preserve activation for the same obligation and recover legacy jobs from exact scheduled evidence | Sync retains activation when identity, target, schedule, and Home match; legacy parsing chooses the earliest matching scheduled receipt before file timestamps | `add_list_remove_round_trips_loaded_launchd_spec`; `legacy_cron_activation_is_recovered_from_its_first_scheduled_receipt` | pass |
| An empty metric population is missing evidence, not zero | Do not synthesize `0 / 0` after auditing other absence-based health claims | The scorecard emits Unavailable with the exact source time and reason; non-empty observations contain only the durable metric fields | `uv run pytest python/tests/test_lifecycle_scorecard.py -k task_loop_trust -q` (4 passed) | pass |

## Source Review

The core model is one `CronObligation`: cron identity, fixed-daily schedule,
Home, activation, and retained receipts. `lf doctor` derives the latest due
interval from that value and makes no writes. Raw ledger events remain a
separate historical diagnostic. No evaluator, compatibility type, or second
continuity state was added.

One review finding was fixed. The installed binary executes
`scripts/lifecycle_scorecard.py` from the selected checkout, so binary and
checkout revisions can legitimately differ. Strict unknown-field rejection
would have made the current binary reject the prior script's harmless
`eligible` and `successful` annotations. The producer no longer emits those
fields, the durable Rust type does not carry them, and the decoder remains
liberal across checkout skew. The focused
`telemetry_envelope_accepts_older_producer_annotations` proof passes.

## Disposition

The behavior and source now match the Task outcome. Publishing remains subject
to Loopflow restoring Task ownership for this retained post-merge worktree.
