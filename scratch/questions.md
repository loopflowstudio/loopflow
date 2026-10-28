# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed repair remains
unverified; Infrastructure owns CLI/helper installation and installed acceptance.

Prior archives and receipts remain in
`775fe090130060fe4cb7a7e898f97dd0ebfd5ce7:scratch/questions.md` and its references.
The compress receipt at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-supvaqza/receipt.json`
verified source absence after checkpoint `775fe0901`; it moved no telemetry.
That reservation is complete and must not be reused.

The prior realign reservation and its receipt remain at
`a2763b171:scratch/questions.md`.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-implement-native-wolw9cry/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.

October 28 implementation assumption: use the library test build's existing mocked
HTTP endpoints and opt only one disposable home per child into the actual native
storage functions. Production gains no test backend or endpoint override. The
native test remains explicit because ordinary unit tests must not touch Keychain;
its eight-case failure is retained, not waived. The design records the excluded
transaction/password candidate and the unresolved writer decision.
