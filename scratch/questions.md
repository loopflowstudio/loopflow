# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No product decision or relaxation of
preservation, trust or storage authority is assumed.

2026-10-27: supported account verification now reports `refresh grant rejected`
for jack@loopflow.studio and requests reconnection (design finding 32). The
headless run does not initiate browser sign-in or bypass it. Writer design and
historical save-failure causality remain unresolved independently of that recovery.

Jack Heart authorized this Task-local LOO-664 sync bridge on 2026-10-27: validate
and checkpoint explicit paths, then atomically archive complete `.lf/tmp` bytes
and leave it absent for automatic sync. Checkpointing alone does not repair
installed19. Preserve new telemetry separately if recreated; never delete logs,
overwrite archives, change ignore rules, force staging or suppress Git errors.
LOO-664 / #1754 is integrated; its disposable reproduction proves only the bridge.
Infrastructure owns CLI/helper installation and installed acceptance.
The previous realign archive exists and preserves that complete telemetry directory:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-realign-rk9ra266/tmp`.
Realign found fresh telemetry after kickoff. Its separate post-checkpoint archive:
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261027-realign-DgnB3mVm/tmp`.
Previous archive paths and references:
`ab1fbeab97536857ec30c5d52ff1248194ea84c0:scratch/questions.md`.

2026-10-28 realign's archive is present at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-3w4gjfu2/tmp`.
The subsequent kickoff found `.lf/tmp` absent and installed `lf --version` still
reporting 0.14.19. No archive was needed at that inspection; no installation or
credential recovery is inferred. Source v0.15.0 is integrated. The writer and
live-acceptance dependencies remain as recorded in the design.
