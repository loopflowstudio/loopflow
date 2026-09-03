# v0.12.17

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.12.17 hardens both sides of an upgrade: the installation users run and the release pipeline that produces it. A failed promotion now leaves the previous `lf` available, interrupted same-tag publication can resume from matching durable state, and the Mac release gate proves the packaged app can render without local build resources. The result is a release path that recovers when evidence is clear and stops safely when it is not.

## Keep using `lf` when an install fails

An interrupted or failed install switch no longer bricks ordinary CLI startup. Loopflow resolves pre-commit switch phases to the recorded last-good installation while preserving the unsettled receipt for diagnosis and a later recovery attempt.

- Ordinary commands fall back to the switch receipt's `prior` install during every unsettled pre-commit phase.
- A committed switch continues to select its intended target.
- `lf doctor` reports the unsettled switch and the active fallback instead of describing startup as blocked.
- Install operations retain the receipt, so they can recover or rerun the promotion without hiding what failed.

## Resume interrupted releases from durable evidence

Same-tag release publication now treats generated worktrees, refs, and artifact directories as process-durable state. Candidate preparation and tagged publication share one exact-source recovery path, allowing an interrupted release to continue without manual cleanup when the existing state can be attributed unambiguously.

- Complete, clean worktrees at the exact tag commit are reused.
- Attributable partial states—including empty paths, branch-only worktrees, and missing release bodies—are reconstructed under the existing stage lease.
- Recovery preserves the exact-tag, exact-commit, verified-artifact, and single-publisher gates.
- Dirty, divergent, differently registered, or live-owned worktrees are left untouched and fail with expected-versus-observed evidence.

## Prove the Mac app stands alone before shipping

The Mac release now tests the assembled application without access to SwiftPM's local build resource bundles. DMG creation proceeds only when the signed packaged app launches in UI-test mode and produces a non-empty snapshot, catching builds that work on the release machine but would fail after installation elsewhere.

- Packaged apps prefer the embedded `LoopflowSwift_Loopflow.bundle`; development and framework resource lookup remain supported.
- Release verification temporarily hides build-time bundles, waits up to 30 seconds for a rendered snapshot, and includes launch diagnostics on failure.
- Hidden resources are restored on every success and failure path.

## Operational notes

- A failed install promotion may still require repair, but ordinary `lf` commands remain available through the recorded prior installation and `lf doctor` exposes the unsettled state.
- Corrupt or ambiguous Git metadata intentionally prevents automatic release recovery. The existing evidence is preserved for operator inspection.
- Mutable release-PR worktrees are not covered by the immutable-source recovery policy.
- Mac release builds now require the packaged app to render successfully in the build environment before DMG creation; the gate verifies startup and initial rendering, not broader interaction.
