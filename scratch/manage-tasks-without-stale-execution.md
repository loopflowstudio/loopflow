# Manage Tasks after delivery — LOO-408

Jack Heart authorized autonomous implementation, verification and delivery.
The design and earlier check evidence are retained at
`796caa84b:scratch/manage-tasks-without-stale-execution.md`.

Task decisions preserve Session turns, reservations, process outcomes and live
controls. Verified merge completes by default; explicit remaining work records
its outcome, evidence condition and next check. Time never establishes success.
Cleanup independently retains occupied or uncertain checkouts. Open PRs and
committed additional work still need delivery or explicit abandonment.

## Delete — do not maintain

The branch removed historical-process acceptance writers/flags, execution scans
from completion, cancellation's execution prerequisite, terminal Tasks hiding
live execution, default keep-open delivery and inferred worker PR rotation.
Retain historical event decoding and exact process controls.

Earlier compression removed duplicate checkout-protection helpers, stored
completion readiness, repeated landing-disposition selection and unreachable Task
action branches. Terminal or canceling Tasks return before parent-PR advice.
The final simplification reuses the completion check's loaded PRs and removes an unreachable
PR-link label fallback. Cleanup errors describe retained work without claiming
the already-recorded cancellation was refused. No further deletion target remains.

## Remaining work

PR #1488 merged as `cead4c952`; PR #1483's Process/LFID vocabulary is integrated
without another migration. PR 2 preserves the simplifications at
`81d3da537` and `303bfc64f`. Implementation review confirmed the retained PR
selection matches the store's active-PR predicate and the preservation regression
covers repeated completion with reserved input, pending turns and live process
receipts. Gate review found no additional implementation gap. Delivery owns
publication of these post-merge simplifications; no further implementation loop is needed.
Installed 0.13.9 still reports LOO-353's five unresolved turns, reserved input
and two unresolved processes. Acceptance requires the first published release
containing #1488, followed by supported completion and preservation readback.
Source tests never mutate the main Home.
Jack Heart's comment `cf9e2775-154a-4f37-86b1-79a42cd5cf49` retires numeric
performance targets, soak requirements and deeper optimization; LOO-304 closed.
Installed acceptance also includes supported settlement of LOO-371/376/375,
preserving the unresolved Session turn and historical Processes identified in
Infrastructure memory. LOO-378 is paused with unpublished code retained, without
deletion or delivery authorization. LOO-285's scheduled settlement and LOO-390's
storage-prevention evidence remain distinct from merge.

Checks: `uv run python scripts/test.py --base 626789dcd --reuse-passing` — architecture/fmt/Clippy PASS; materialized Rust 2,288 PASS, 2 FAIL, 17 skipped; unchanged account reconnect (5 s startup) and screenshot (500 ms startup) fixtures both PASS individually under `scripts/test_network.py` using the same test binary; full concurrent pass remains CI-owned, installed acceptance awaits publication.
