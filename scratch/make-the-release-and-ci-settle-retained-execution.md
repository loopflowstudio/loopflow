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

Prevention remains unimplemented. Source reconciliation on October 5 confirms
the receipt lifetime crosses four removal/replacement paths:

- Terminal emission in `journal/mod.rs` removes the receipt even when the ledger
  write fails.
- `write_exec_process_receipt` replaces receipts keyed by reusable PID.
- `run_prune` in `lf/commands/top.rs` removes stale receipts.
- `ensure_exec_context` registers receipt removal as interrupt cleanup.

These paths are not a proven cause of this incident. The required regression
starts with a recorded, unfinished Exec, fails its terminal write, then exercises
interrupt cleanup, PID reuse and pruning without losing its exact identity.
Existing write-lock tests cover a missing start and later ledger recovery; they
do not prove preservation of an already-recorded Exec's process evidence.
Live identities and failed process observations must still block completion;
confirmed death must preserve the absent outcome and history. Prevention cannot
recover the missing identity retroactively or by itself complete this Task.

## Preservation boundary

The replacement for the loss-prone identity lifetime is still a design choice;
no schema or storage mechanism has been selected. There is no read-only-command
exemption. Release's October 4 retained-landing incident supplies a useful
counterexample: an exact receipt proved death without a terminal outcome, while
separate release leases established re-entry authority. Process evidence must
survive cleanup; it does not replace operation-specific lease ownership.

Prior readback found no exact exit evidence; monitor prune dry-run reported zero
errors. No new runtime observation or Task settlement is established here.

Check: `git diff --check` — passed; source review confirmed four identity-loss paths; prose-only reconciliation, no behavioral tests rerun.
