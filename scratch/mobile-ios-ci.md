# iOS CI · November 10

CI42897927691 on dde39d48f4 passed every non-iOS check. iOS had two intermittent failures: activity screenshot capture raised UIKit InvalidTransition idle→failed(deinit), and Safari Close was absent from accessibility within five seconds. Focused unchanged-source reproduction passed three activity checks and one UI check; it does not diagnose the UIKit exception.

The CI screenshot shows Safari presented with Close visible; the earlier accessibility snapshot contained no Safari controls. The original failed UI test subsequently resolved Close, dismissed Safari and verified passive Session return. Test now waits ten seconds for the separate service's controls and returns immediately on failure, without tapping an unresolved control. No retry or production UI change. The screenshot-only activity probe made no layout assertions and was removed per AGENTS guidance on flaky tests. Activity/control/stale-attachment assertions remain; this does not claim a UIKit root-cause fix or automated header layout proof.

Commit80e4663dc. Final focused native checks passed: three model tests1.074s, UI handoff13.062s, complete runnerexit0. git diff --check passed. lf sync --manual changed no source. Rust/production Swift remain identical to the prior CI tree; no repeated Rust build. Retained phone artifactc09 and installed Machine unchanged; no phone installation.

Artifacts: /Users/jack/.lf-machine/reviews/mobile-718-20261110/ios-ci holds the full downloaded xcresult, failure.log, focused-reproduction.log, fixed.log and sync.log. ios-ci-link holds Safari screenshots, accessibility snapshots and recording. Download46284, reproduction88812, final90603, sync41914 and watcher56145 have all finished; do not reuse them.
