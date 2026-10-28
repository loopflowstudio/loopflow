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

The earlier realign archive completed at:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-ojie9_hk/tmp`.
Its receipt records checkpoint `55054cd70c37da378c956c6ddbfce982762649ca`,
exact byte verification and source absence at that move; later telemetry is separate.

The current realign archive destination is `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-4yodp0fv/tmp`.
Its sibling `receipt.json` records the checkpoint, byte verification and source
absence after the move; the destination alone establishes neither archiving nor
installed repair.

Earlier archive receipts, the unused kickoff reservation and the last installed
0.14.19 observation remain at
`d498137af513522569d19da82e4b26c33656687b:scratch/questions.md`.
Those archives remain intact. Integrated source v0.15.0 proves no installed or
credential recovery.

This kickoff reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-kickoff-znbdhg6v/tmp` for the authorized telemetry bridge.
Its sibling `receipt.json` records completion and the checkpoint; a reservation
alone proves no archive or installed repair.
