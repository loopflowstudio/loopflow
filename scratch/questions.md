# LOO-661 open questions and assumptions

Jack Heart asked on October 28 whether help was needed or the repair could be
fixed directly. Containment remains a draft requiring a product-scope decision;
the existing repair acceptance and shipping authorization do not select it.
The [design](loo-661-save-refreshed-claude.md#containment-alternative--draft-not-selected-october-28)
owns its behavior, tradeoff, upstream reports, unresolved writer mechanism and
live diagnosis. No preservation, trust or storage constraint is relaxed.

The native fixture uses existing mocked HTTP endpoints and opts only one
disposable home per isolated child into actual storage. Production has no test
backend or endpoint override. Ordinary tests stay on files; the explicit native
fixture's four large-value failures remain binding evidence.

## Task-local sync recovery

Jack Heart authorized the LOO-664 bridge: checkpoint explicit paths, then
atomically archive complete `.lf/tmp` bytes to a unique directory beneath
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/` and leave
the source absent for automatic sync. Preserve later telemetry separately;
never overwrite archives, change ignore rules, force staging or suppress Git
errors. Infrastructure owns coordinated CLI/helper installation and installed
acceptance; neither is established by source integration or this bridge.

Prior conversation observations, assumptions and archive reservations remain at
`166809f6a4c089e6a13681a71270b966ee42b733:scratch/questions.md` and its references.
Do not reuse reservations. Each archive's sibling `receipt.json` records the
checkpoint and verified move, or source absence; a reservation proves no archive.

This realign step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-03mf_7z0/tmp` for telemetry after checkpointing.
Its sibling `receipt.json` records the verified move or source absence.
