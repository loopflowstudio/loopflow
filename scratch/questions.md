# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved compatible-writer
mechanism, rejected alternatives and live diagnosis. No product decision is
pending and no relaxation of preservation, trust or storage authority is assumed.

2026-10-27: Jack Heart's current Task definition authorizes archiving the complete
Task-local `.lf/tmp` after validation and an explicit-path checkpoint, leaving it
absent for automatic sync. Checkpointing alone does not restore installed19 sync.
The preceding realign archive is
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-realign-hxkX9Mtm/tmp`.
Keep earlier archives and new telemetry separate; no logs are deleted or
overwritten. No ignore-rule change, forced telemetry staging or suppressed Git
error is authorized.
This bridge changes neither writer proof nor Infrastructure's installation and
live-acceptance ownership. Earlier archive paths remain at
`22d5d8e0e5:scratch/questions.md` and `0288f9d611:scratch/questions.md`.
