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

The prior iteration's “prevention remains unimplemented” finding is superseded
by the implementation and focused proofs below. Remaining work is CI verification after the host-interrupted local gate,
delivery and installed acceptance, plus the independent retained Exec recovery. Prevention alone does not satisfy Task completion.

Recover an independently retained PID/start-time receipt for the existing
liveness reader, or observe a subsequent machine boot: `exec_process_evidence`
in `rust/loopflow/src/journal/mod.rs` recognizes pre-boot execution as dead
without inventing an outcome. Neither proof exists in the recorded observation.
Preserve unknown status. Rebooting is outside this pass; Task completion must
run outside this Task's active contribution, whose own Exec gates completion.

## Implemented prevention — October 5, 2026

The existing receipt owner now uses Exec IDs rather than reusable PIDs. Normal
completion and interruption remove identity only after the terminal ledger write
succeeds. Prune requires both exact process death and a matching persisted
terminal record; missing records and failed reads retain identity. Existing
receipt filenames remain readable. No schema change or historical outcome repair
is involved. These loss paths are not proved causes of the original incident.

Unconditional terminal/interrupt receipt removal and PID-keyed replacement are
removed. Current process observations, terminal history and operation-specific
lease checks remain authoritative.

Compression keeps terminal receipt removal beside the successful ledger write,
so ordinary completion and interruption share one retention rule. Prune opens
the ledger and scans receipt files once for all selected PIDs, then independently
checks each receipt’s exact identity and terminal record.

The regression begins with a recorded unfinished Exec, fails its terminal write,
then exercises interruption, another Exec using the same PID, failed process
observation and pruning. A mismatched birth identity establishes death without
changing the stored record. The public prune regression covers both filename
formats and retains unfinished history while removing settled dead receipts.

## Remaining verification and recovery

The affected gate reached 1,903 Rust passes before host security pressure
stopped it; CI owns the unfinished verification. The source repair does not
establish installed behavior or recover the existing missing identity. Release's October 4 retained-landing incident remains relevant: exact
process evidence proved death, while separate leases established re-entry
permission. Neither Session closure nor a known process exit supplies an outcome.

Review finding: prune previews previously promised removal for all stale PIDs.
They now identify candidates and explicitly retain unfinished receipts; actual
removal counts remain separate.

October 5 reconciliation inspected Release's complete child memory and objective.
Its slow-transfer recovery distinguishes the total artifact deadline from retry
policy; that release-owned fix does not change this receipt repair's scope.
No implementation mismatch was found; existing focused results remain applicable.

Gate review found no further code or documentation mismatch. The changed retention,
public prune and interruption regressions passed under external-network isolation.
The runner stopped four active tests after syspolicyd exceeded its pressure
threshold; 333 tests never ran. These are interrupted results, not established
product defects or passes. Preserve the failed gate receipt; capable CI must
finish the Rust suite. Logs: `.lf/tmp/gate/run-56255/rust/rust.log`.

Check: `uv run python scripts/test.py --base 8ea0bec9cf4b0c08ca17c52e57de059000a7b0e3 --reuse-passing` — architecture, fmt, all-target Clippy and website (78 passed, 3 skipped) passed; release-materialized Rust recorded 1,903 passes, 4 SIGTERM interruptions, 333 not run and 17 skipped under host security pressure; Rust completion deferred to CI; inherited LF_/LOOPFLOW_ authority cleared and LF_BIN pinned to this checkout.
