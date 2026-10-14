# macOS CI critical path

October 13, 2026 — lf-ddf2d53. Jack Heart authorized continued implementation
and delivery. Accepted outcome: useful macOS critical-path margin on matched
source-cold and changed-source warm inputs, including preparation and cleanup,
without material warm regression or duplicated compilation. Maintenance cost
matters; every warm sample need not improve. The selected implementation is
complete locally; performance acceptance and delivery remain unproved.

The supplied Task brief selects same-source CLI/Xcode overlap only on exact-source
DerivedData misses. Implemented in `scripts/build_xcode_ci.py`; the existing
`xcode-cache` result selects the path. Hits execute ordinary Xcode directly.
Preparation and post-build copy share `_build_control_tools` at
`target/dev-app-control`; Xcode still verifies freshness, copies and signs.
SDK/deployment come from Xcode settings; preparation matches the app shell's
Cargo path. Both process failures propagate. Cancellation stops owned process
groups and scoped DerivedData compilers, preserving unrelated/pre-existing work.

Local proof now supports gate and hosted evaluation, not performance acceptance:

| Matched pairs, both orders | Combined saving |
|---|---:|
| Source-cold misses | 4.794s (individual -1.918s / +6.712s) |
| Changed-source warm misses | 18.575s (+12.024s / +6.551s) |
| Exact-source hits | -0.064s (-0.096s / +0.032s) |

All 12 signed Xcode app/test-runner builds passed; helper and app signatures
verified. Each cold/warm sample compiled `loopflow` once. Miss candidates showed
concurrent Rust/Swift work and post-build `Fresh loopflow`; hits compiled it zero
times and launched no preparation. Shared services and concurrent unrelated builds
limit attribution, especially the mixed cold result. Actual hosted benefit and
whether it justifies cleanup cost remain unsettled.

Ten network-isolated process tests passed. Real simultaneous Rust/Swift
cancellation returned 143 in 5.123s, leaving no checkout compiler. Disposable,
network-isolated authority checks matched development/validation-only provenance
and current source identity. A raw stripped-binary hash check failed because
codesign retained different `__LINKEDIT` virtual sizes; removing and reapplying
identical ad-hoc signatures to disposable copies proved identical bytes. No
installation preflight or installed Home mutation occurred. Full Swift execution
and installation coverage remain gate/CI owned.

[Durable evidence and reproduction](../docs/reviews/ci-under-five-minutes.md#october-13-follow-up-exact-cache-miss-overlap)
retains every sample, resource accounting, exclusions and corrected signing proof.
Raw replay: `/tmp/macos-ci-selected-bench.py` and
`/tmp/macos-ci-selected-snapshots/results.jsonl`. Source baseline `39651041f8`;
all temporary source edits and ordinary output snapshots were restored.

The inline Cargo build in `_copy_bundled_tools` moved
into `_build_control_tools`; CI's inline Xcode argument list moved into the single
wrapper command. No parallel build implementation remains. Rejected always-overlap
wiring, codegen-units override and boxed `sync_repository` remain absent. The
single async sync function, await ordering, timeouts and acquisition tests remain.
Compression removed duplicate process-label reconstruction, the fixture's unused
no-descendant branch, and manual Cargo status checking. Named children retain
cleanup ownership after completion; the existing checked command runner now
reports Cargo's failed command and status. Scoped compiler cleanup remains
necessary because process groups alone failed the real cancellation trial.

Historical trials remain in the same durable review: always-overlap cold gains
13.453–39.668s / warm 0.417s slower combined; package-only codegen cold 1.836s
slower; boxed future cold 0.921s and warm 0.446s slower. Frontend attribution found
distributed cost and justified no small frontend rewrite. Those rejections are
not overturned. The selected variant changes cache-hit behavior, and its current
cold result must not inherit the earlier saving as a forecast.

October 13 reconciliation at `a506a65fd5`: implementation, selector wiring and
recorded local proof agree. Infrastructure memory now reflects implementation;
its Release child's operation-entry and cold-acquisition lessons still apply.
Main through `414d4dd27c` includes #1545's installation image acquisition repair;
that merge establishes neither this candidate's installation proof nor CI margin.

Remaining: gate once with full Swift execution and installation/network coverage,
then full hosted CI/queue, measured useful margin and verified delivery. Installation
proof requires disposable OS accounts; `LF_HOME` alone does not isolate preflight.
Hosted observations must distinguish cache hits/misses, helper compilation and
overlap from download/scheduling time. Cold sensitivity and cleanup maintenance
cost remain acceptance concerns, without a new numeric threshold or product choice.
No paid runners, test deletion, relaxed cache identity or arbitrary retries.
Ordinary signed Xcode proof, current source/target, copy/sign, development provenance
and validation-only migrations remain required. Completion requires both a
qualifying measured outcome and verified merge. [Aggregate proof](ci-remaining-proof.md) remains
lf-12be237's independent ten-real-run and queue/push-to-merged obligation. The latest
supplied real queue remains 6:24; no numeric acceptance is waived.

Check: `git diff --check` PASS; prior `uv run --no-sync python scripts/test_network.py uv run --offline --no-sync pytest python/tests/test_build_xcode_ci.py -q` — 10 passed, reused unchanged code; gate owns full Swift/installation, CI owns hosted/queue proof before delivery.
