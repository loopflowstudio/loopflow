# LOO-661 open questions and assumptions

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed repair remains
unverified; Infrastructure owns CLI/helper installation and installed acceptance.

Prior reservations, archive paths and receipt references remain in
`ed6c82b2bc02e3f9b0d6f3ca21a5a15179e33f4f:scratch/questions.md`.
Do not reuse any prior reservation.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-compress-se02u2sn/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.

October 28 implementation assumption: use the library test build's existing mocked
HTTP endpoints and opt only one disposable home per child into the actual native
storage functions. Production gains no test backend or endpoint override. The
native test remains explicit because ordinary unit tests must not touch Keychain;
its eight-case failure is retained, not waived. The design records the excluded
transaction/password candidate and the unresolved writer decision.
