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
Reserved post-checkpoint archive:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-compress-LvZUVt4w/tmp`.
Previous archive paths and references:
`ab1fbeab97536857ec30c5d52ff1248194ea84c0:scratch/questions.md`.
