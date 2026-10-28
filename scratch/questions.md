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
The realign receipt confirms byte preservation at checkpoint `920e414455`;
`.lf/tmp` was absent at this compression inspection. Archives remain untouched.

This pass reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-bxuuwerb/tmp`
for any fresh telemetry created by checkpointing. Its sibling `receipt.json`
records the checkpoint and verified move, or source absence if no move is needed.
A reservation alone proves no archive; later telemetry remains separate.
