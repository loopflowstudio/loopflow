# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

Findings 36–37 record the missing supported authorization/encoding mechanism;
the local-authentication-context review identified no bridge in the file-keychain path.
No scope relaxation is assumed; a database password alone does not resolve it.
Do not collect a secret speculatively or repeat the rejected mutation probes.

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
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-ib29t40r/tmp`.
Earlier receipts and archive references:
`9cdf9bb872178d0c5e4a09cb1eb1fa821e7ff090:scratch/questions.md`.
Installed `lf --version` reports 0.14.19; integrated source v0.15.0 proves
no installed or credential recovery.

Realign checkpoint `95d012ef038770a3114f31728899694c1d110282` preceded the atomic archive;
all 1 telemetry files match their pre-move digests, `.lf/tmp` was absent
after the move, and prior archives remain intact. This proves telemetry preservation only.

Kickoff archive destination (2026-10-28), reserved before checkpointing these notes:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-kickoff-a4trWWUW/tmp`.
The post-checkpoint command receipt records the atomic move and byte verification;
this reservation alone proves neither archival nor installed recovery.
