# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
Reserved post-checkpoint archive:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-compress-vgc7lpb_/tmp`.
Previous archive paths: `e3f012685c:scratch/questions.md`,
`8e52d2e163:scratch/questions.md`, `22d5d8e0e5:scratch/questions.md` and
`0288f9d611:scratch/questions.md`.
The disposable reproduction proves the bridge only. Infrastructure owns the
prepared CLI/helper installation and live acceptance.
