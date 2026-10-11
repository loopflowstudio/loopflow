# Cron conversion review — 2026-10-10

For the accepted cron cut in [the design](work-on-another-machine-name.md).
Source inspection at `582c134439`; no reproduction or live schedule change.

`ops/cron/accounting.rs::{claim_execution,validate_execution}` hashes
`(canonical repo, wave, flow)`. Child validation compares the inherited lock
descriptor's device/inode and proves continued ownership. Replacing this key with
an argv hash or fresh schedule ID could split exclusion or reject a surviving
release child. Preserve this boundary when removing Wave-specific definitions.

`cron::label`, plist readback, obligation segments and telemetry matching retain
Wave/Flow identity; telemetry currently requires a Flow receipt. Conversion must
preserve original activation, due, Machine and receipt classification.

Recommended proof: convert an isolated installed schedule while its execution
lock and inherited child descriptor remain held. The converted command must
preserve exclusion and child validation, original dues and historical receipt
meaning. Reading a cloned configuration activates nothing. Fold this evidence
into the cron implementation plan; no new product decision is proposed.
