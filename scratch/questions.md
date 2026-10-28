# LOO-661 open questions and assumptions

## Direct conversation, October 28

Jack Heart asked whether help was needed or the repair could be fixed directly.
Fresh status is idle with no live Graph in the checkout; the existing native
fixture and Unicode reader repair are retained. No new Graph or credential
operation was launched.

A concrete containment option is awaiting Jack Heart's product decision:
in `fresh_claude_token`, native Keychain homes would return sanitized
`Unavailable` before any OAuth refresh when expiry or a 401 requires refresh.
Unexpired native tokens still supply usage; the next poll rereads credentials
after Claude itself refreshes them. Managed file-backed accounts keep their
existing refresh behavior. No background Claude inference, forced sign-in,
account-route change, credential migration or Keychain mutation is proposed.
Tests would prove zero refresh requests and byte-for-byte unchanged native
credentials on expired/401 paths, plus recovery on a subsequent poll after an
external replacement. The activation writer's large-value defect would remain
open; this option does not satisfy the Task's existing arbitrary-size write
acceptance and must not be reported as the full repair.

Upstream reports corroborate that provider-owned refresh alone is not a proved
solution: anthropics/claude-code issues #28541 and #30337 describe the same
`security -i` size corruption; #94491 reports reader-access loss after refresh.
These are reporter observations, not verified behavior of installed Claude
2.1.312 or proof of this account's historical failure. Sources:
https://github.com/anthropics/claude-code/issues/28541
https://github.com/anthropics/claude-code/issues/30337
https://github.com/anthropics/claude-code/issues/94491

The [design](loo-661-save-refreshed-claude.md) owns the unresolved writer mechanism,
rejected alternatives and live diagnosis. No relaxation of preservation, trust or
storage authority is assumed.

Jack Heart authorized the Task-local LOO-664 sync bridge: checkpoint explicit
paths, then atomically archive complete `.lf/tmp` bytes and leave it absent for
automatic sync. Preserve later telemetry separately; never overwrite archives,
change ignore rules, force staging or suppress Git errors. Installed repair remains
unverified; Infrastructure owns CLI/helper installation and installed acceptance.

Prior reservations, archive paths and receipt references remain in
`2b2079af4b98b6c223b1d057f33b834476cca616:scratch/questions.md`
and its prior references.
Do not reuse any prior reservation.

This step reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-realign-cdy57_n0/tmp`
for fresh telemetry after checkpointing. Its sibling `receipt.json` records the
checkpoint and verified move, or source absence. A reservation alone proves no archive.

October 28 implementation assumption: use the library test build's existing mocked
HTTP endpoints and opt only one disposable home per child into the actual native
storage functions. Production gains no test backend or endpoint override. The
native test remains explicit because ordinary unit tests must not touch Keychain;
its eight-case failure is retained, not waived. The design records the excluded
transaction/password candidate and the unresolved writer decision.

October 28 kickoff: the existing repair scope remains binding; containment is
a draft, not implied approval to reduce acceptance. Source and upstream review
added no eligible writer mechanism or live recovery evidence. The design now
records its exact behavior and tradeoff.

This kickoff reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-661-20261028-kickoff-cz0a4g61/tmp`
for telemetry after the explicit-path checkpoint. Its sibling `receipt.json`
records the verified move or source absence. Earlier reservations remain historical;
this reservation alone proves no archive.
