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
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-9fp_5nmp/tmp`.
Earlier receipts and archive references:
`5b00d2da7a:scratch/questions.md`.
Installed `lf --version` still reports 0.14.19; integrated source v0.15.0 proves
no installed or credential recovery.

Realign checkpoint `9c7391073ce1cf8581f97202868a1edafcc9440d` preceded the atomic archive;
all 1 telemetry files match their pre-move digests, `.lf/tmp` is absent,
and prior archives remain intact. This receipt establishes telemetry preservation only.
