# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.
The 2026-10-27 kickoff's disposable prefix experiment establishes failed-save
data loss in the existing transport (design finding 30); it supplies no live-account
cause or compatible replacement mechanism.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
LOO-664 / #1754 is integrated in this checkout; installed recovery remains
Infrastructure's separate acceptance. Reserved post-checkpoint archive:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-kickoff-wh4l2lLf/tmp`.
Previous archive paths: `ed0f49d4f6:scratch/questions.md`,
`3c8b9cb364:scratch/questions.md`, `e3f012685c:scratch/questions.md`,
`8e52d2e163:scratch/questions.md`, `22d5d8e0e5:scratch/questions.md` and
`0288f9d611:scratch/questions.md`.
The disposable reproduction proves the bridge only. Infrastructure owns the
prepared CLI/helper installation and live acceptance.
