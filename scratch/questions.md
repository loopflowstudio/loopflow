# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
LOO-664 / #1754 is integrated; its disposable reproduction proves only the bridge.
Infrastructure owns CLI/helper installation and installed acceptance.

This realign pass reserves the telemetry archive destination:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-ojie9_hk/tmp`.
The sibling `receipt.json` records the checkpoint, byte verification and source
absence after the atomic move. Reservation alone proves no move or installed repair.

Earlier archive receipts, the unused kickoff reservation and the last installed
0.14.19 observation remain at
`d498137af513522569d19da82e4b26c33656687b:scratch/questions.md`.
Those archives remain intact. Integrated source v0.15.0 proves no installed or
credential recovery.
