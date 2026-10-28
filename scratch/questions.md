# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed repair remains
unverified; Infrastructure owns CLI/helper installation and installed acceptance.

Prior archive paths and receipts remain in
`d030ff821795bc2cdb57dc8b5c3828eaa0a0aba8:scratch/questions.md` and its references.
The compress receipt at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-6l3nxoyh/receipt.json`
records checkpoint `d030ff821` with `moved: false` and source absent; it proves no telemetry move.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-0v453nev/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.
