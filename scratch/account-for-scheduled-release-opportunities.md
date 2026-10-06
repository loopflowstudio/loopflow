# October 6 telemetry repair

The scheduled release cron_a4b8b11b2a534bf99d183e677f2a6871 failed after
recovery cron_5d930c31c7ae4a118f6b93774496991c passed continuity but emitted
an observation for the retired product/task-loop-trust contract. The accepted
September chapter removed that contract in 42451654e; the Python producer
still emits a hardcoded unavailable observation.

Remove that obsolete producer output, retaining the typed empty observation
list and all lifecycle report rows. Rust contract/publication failures remain
strict. Prove the real producer succeeds with no retired contract and still
reports measured lifecycle evidence. Missing database/history/policy must fail.
Historical observations and failed receipts remain unchanged. No synthetic or
manual recovery establishes two adjacent original scheduled settlements.

Jack Heart authorized source repair publication and landing; the operator owns
release recovery after LOO-382/PR #1452. Do not launch release or install here.
Preserve the pending version; report supported installed commands after checking
current recovery semantics. Existing installer-isolation changes remain intact.

Verification: `uv run pytest python/tests/test_lifecycle_scorecard.py -q` 12 passed; Ruff passed; installed `lf __telemetry-scorecard --json` against this checkout returned 35 rows. No release launched.

Review: deleted the retired producer rather than weakening Rust contract errors;
missing database/history/policy still fail without an envelope. No storage/schema
change. PR #1457 already merged; supported rotation created PR 3.
