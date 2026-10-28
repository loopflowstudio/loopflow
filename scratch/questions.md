# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed repair remains
unverified; Infrastructure owns CLI/helper installation and installed acceptance.

Prior archive paths, reservations and receipts remain in
`108095b26cfe5950c7ce42b6115e936601547ede:scratch/questions.md` and its references.
The previous kickoff receipt at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-kickoff-itu_h7r8/receipt.json`
records source absence after checkpoint `108095b26`; no telemetry was moved.
That reservation is complete and must not be reused.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-h7sbaww9/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.
