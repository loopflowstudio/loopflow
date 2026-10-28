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
`95ffdead7d31857dc6f960d4085cb80b50bd5b7e:scratch/questions.md` and its references.
The previous compression receipt at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-h7sbaww9/receipt.json`
records source absence after checkpoint `95ffdead7`; no telemetry was moved.
That reservation is complete and must not be reused.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-8zcoxtyj/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.
