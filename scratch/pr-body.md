Required CI previously took roughly 20 minutes. This change removes repeated build/fixture work and overlaps independent proofs on ordinary runners. The retained implementation passed the full Actions workload in 4:25 on the current head (and previously in 4:55). An earlier 4:56 Actions pass took 5:00 from push. Ten consecutive real sub-five-minute runs, merge-queue timing and push-to-merged remain unproved.

<!-- loopflow:task-pr-context:start -->
> [!NOTE]
> **Task:** Get CI under five minutes end to end · lf-12be237
> **PR lifecycle:** Merge delivers the code. Record independent later work as Tasks and comments; complete the delivered outcome.
<!-- loopflow:task-pr-context:end -->

## What changes

- Separate compiled provenance from Cargo inputs, reuse prepared CLIs, and compile installation targets together. Retain a distinct published-title build and disposable OS-account/Home authority. Cache dependency files, never installation verdicts.
- Restore independent prepared fixtures for all 70 command/environment cells; hosted execution fell from 105.311s to 10.085s.
- Compile and archive Rust once. Consolidate 41 ordinary integration targets into one executable and one shared fixture module; keep fourteen dedicated proof/environment targets. Every original case remains, with module-qualified names.
- Download separate library and integration archives. Integration consolidation reduced its archive from 380 MB to 196 MB; aggregate downloads across the same nine execution jobs fell to about 1.04 GB.
- Run five library and three integration partitions with six test slots on the same hardware. Partition zero overlaps the four existing quietness cases with checkout refresh; it waits for both commands and fails on either failure. Quietness observation windows and assertions remain unchanged.
- Keep reconnect proofs in the smoke job and macOS process discovery separate from Swift. Omit CI Rust debug symbols, retaining unoptimized code and debug assertions. Seed missing dependency caches during PR review.

## Reliability repairs and coverage changes

No test or job is removed or demoted. Confirmation runs exposed incidental assumptions now replaced with behavioral checks:

- Select the rotation Task by identity rather than display position.
- Verify deletion advances planning metadata, then that rejected placement preserves the entire Task.
- Verify shared rate limiting against a future deadline, then expire the stored nonzero timestamp before checking resumption; no 250 ms race or sleep is required.
- Replace the arbitrary five-frame cap over twenty database transactions with strictly advancing Task counts and the retained full fresh-read comparison. Fixed frame-count coverage is explicitly retired because write duration is unbounded; convergence and duplicate-state detection remain.

## Evidence and limits

[Run 38422834854](https://github.com/loopflowstudio/loopflow/actions/runs/38422834854) passed the consolidated layout at `19544d4047` in **5:21**. Integration partitions finished by 4:57, but a library partition finished at 5:15. Build/archive took 110s, versus 124s previously. Runner occupancy was 43:36 versus original baseline 46:44; these are mixed-platform seconds, not dollar charges.

[Run 38424417775](https://github.com/loopflowstudio/loopflow/actions/runs/38424417775) finished in **4:39 but failed** the two timing assumptions above. Its ten-slot experiment improved the limiting partition only from 119.174s to 117.204s and is reverted. The independently successful quietness/checkout overlap remains. This failed run supplies no successful acceptance. The repaired implementation subsequently passed the complete workload in [4:55, run 38427516700](https://github.com/loopflowstudio/loopflow/actions/runs/38427516700). Its exact push timestamp is unavailable. Total runner occupancy was 40:19, compared with 46:44 at baseline; these are mixed-platform seconds, not dollar charges.

An Xcode aggregate-target/parallel-build experiment passed but took [5:44 Actions / 5:49 from push](https://github.com/loopflowstudio/loopflow/actions/runs/38430532823), with a cold DerivedData cache and no established overlap saving. It is removed: the current implementation restores the proven app build graph. Checkpoint run 38430369254 skipped the product matrix and is excluded.

Local consolidation validation preserved all 369 moved cases and ignored flags; all passed, followed by four representative shared-fixture checks. Formatting, all-target Clippy, 45 CI-proof/release-automation tests, actionlint and both-command failure propagation pass. Repair-specific evidence is recorded in `docs/reviews/ci-under-five-minutes.md`.

No premium hardware or paid platform is introduced. Release/nightly workflows, signing and local Cargo profiles are unchanged. CI backtraces lose source-line detail. The broad matrix error classifier and an earlier unconfirmed local Mac output-handle warning remain documented; neither is declared repaired. Merge is unarmed and the Task remains open for repeated-run acceptance.

Latest confirmation: [run 38431676003](https://github.com/loopflowstudio/loopflow/actions/runs/38431676003), current head `2cc041cac2`, passed every required job in **4:25**, 16:34:54–16:39:19 UTC on October 13. Xcode finished at 2:51; installation was last at 4:20. This restores the exact code/workflow of the earlier 4:55 pass; only research documentation differs. Ten consecutive real runs, merge-queue and push-to-merged remain unproved.
Publication began at 2026-10-13T16:34:39Z; publication-start-to-checks was 4:40, an upper bound on push-to-checks. The exact GitHub PushEvent timestamp was not observed.

Main synchronization added five PR-review/scratch cases. They are now included in the consolidated target and pass locally. CI and the local Rust runner reject unregistered top-level test source files before compilation. Formatting, all-target Clippy, Python lint, 45 local-runner checks, 45 CI-proof/release-automation checks, and actionlint pass. The synchronized/repaired head requires fresh full CI; older timing passes are not proof for its new content.

Merge-queue evidence: run 38445438182 failed in 6:43 on an immediate release-tag lock reacquisition. A Git 2.55 disposable-remote experiment retained the inherited lock after push in 3/40 samples with receiver maintenance, versus 0/40 with it disabled. TestRepo now disables automatic receiver housekeeping on its disposable bare remote; production locks and remotes are unchanged. Git 2.55 tag idempotency, conflicting-tag refusal and surviving-child exclusion pass; formatting and all-target Clippy pass. No retry or weaker assertion is added.

The queue missed the new shared-Rust and app-CLI dependency caches. Shared compilation took 174s; Xcode's CLI compile took 256s. PR-scoped cache entries do not establish a warm merge queue. The first main run must seed the new cache namespaces. Ten consecutive real-run and merge-queue acceptance remain outstanding; this repair requires full hosted proof.
