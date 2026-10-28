# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

Supported reconnection for jack@loopflow.studio requires browser sign-in (design
finding 32); this headless run does not initiate or bypass it.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
LOO-664 / #1754 is integrated; its disposable reproduction proves only the bridge.
Infrastructure owns CLI/helper installation and installed acceptance.

Latest archive (2026-10-28 realign):
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-s3p31b6k/tmp`.
Earlier complete receipts and archive references:
`78c41329c831ac9a1cc045dbd745417d434c5e4a:scratch/questions.md`.
Realign still observes installed 0.14.19; integrated source v0.15.0 proves no
installed or credential recovery.
