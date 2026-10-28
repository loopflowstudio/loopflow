# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed `lf 0.14.19`
still needs this bridge; integrated source proves no installation or recovery.
Infrastructure owns CLI/helper installation and installed acceptance.

Prior archive paths and receipts remain in
`083c86ec350c5a51aab763faa39ef2ecd769b8f6:scratch/questions.md` and its references.
The preceding realign receipt at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-v6cd6ra1/receipt.json`
records checkpoint `7f067d39fb38014cffc0a56a084f3d44a517ab72`, a completed move
and verified byte preservation. Earlier archives remain untouched.

This kickoff reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-kickoff-vu9c27jf/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.

October 28 kickoff retains the accepted constraints and existing design. There is
no new supported writer candidate to probe. Browser reconnection remains a separate
live-diagnosis dependency; no permission to relax access or failure preservation is assumed.
