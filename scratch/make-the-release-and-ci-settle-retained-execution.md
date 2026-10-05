# Retained Exec completion boundary

## Evidence — October 5, 2026

Jack Heart requested LOO-326 recovery without fabricated success, raw-store
edits, lost history or weaker live/unknown process protection. His steer records
PRs #1413/#1435 installed in v0.13.3, both stopped recovery Flows ended and the
headless Session completed. The prior readback confirmed installed CLI 0.13.3.

`lf monitor show 5f239ead-89f9-49c4-92c4-4c2f8b97ca94` retained
`task status LOO-326 --json`, started at 1791175322, without terminal fields or
an exact runtime process receipt. It began after the observed machine boot.

The local journal at
`.lf/journal/traces/da30b415-2918-45bb-b3a0-55877d8b7525/events.jsonl`
contains two status-command starts before a completion and later commands.
Events lack Exec IDs, so none establishes this Exec's exit. Caller Session
closure, trace completion and elapsed time do not establish child death.

## Remaining work

Recover an independently retained PID/start-time receipt for the existing
liveness reader, or observe a subsequent machine boot: `exec_process_evidence`
in `rust/loopflow/src/journal/mod.rs` recognizes pre-boot execution as dead
without inventing an outcome. Neither proof exists in the recorded observation.
Preserve unknown status. Rebooting is outside this pass; Task completion must
run outside this Task's active contribution, whose own Exec gates completion.

Prevention must preserve exact identity across three source paths:

- Terminal emission in `journal/mod.rs` removes the receipt even when the ledger
  write fails.
- `write_exec_process_receipt` replaces receipts keyed by reusable PID.
- `run_prune` in `lf/commands/top.rs` removes stale receipts.

These paths are not a proven cause of this incident. Test failed terminal writes,
PID reuse and pruning together; prevention cannot recover the missing identity
retroactively. No source repair or Task completion is established.

## Delete — do not maintain

No concrete deletion target selected. Replace the loss-prone identity lifetime
as one preservation-tested change; retain history and live/unknown blocking.
Do not add a read-only-command exemption.

Prior checks: `lf monitor show` retained unknown outcome; receipt/journal inspection
found no exact exit evidence; `lf monitor prune --dry-run --json` reported zero
errors without mutation. This prose-only compression requires no build.
