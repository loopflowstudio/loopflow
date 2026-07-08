# 5 Whys: Loopflow macOS Security Popups

## The Problem

Running Loopflow UI tests or app builds could produce macOS security popups for damaged UI-test runners or Keychain access, breaking the developer and user flow.

## Chain

Problem -> unsigned or untrusted macOS runners plus interactive Keychain access -> wrappers and docs normalized broken signing settings -> release/dev/test paths optimized for compile success rather than launch behavior -> no regression guard asserted "no security UI" as a product requirement -> UX regressions in operational tooling were treated as local annoyance instead of gate failures.

**Problem**: `LoopflowUITests-Runner` and old `ConcertoUITests-Runner` showed "damaged and can't be opened"; Loopflow also prompted for `loopflow.connection.token` Keychain access.

**Why 1**: The Xcode UI-test runner was built through paths that disabled code signing, or reused stale DerivedData runners signed with identities no longer trusted on the machine. Keychain reads also allowed macOS to show authentication UI.
↳ *Could we have caught this earlier?* Yes: `codesign --verify` on the generated runner already reported trust/signing problems.

**Why 2**: `scripts/test.py`, CI, and docs carried `CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` for the Loopflow UI target. Screenshot generation also called `xcodebuild test` directly without a shared signing policy.
↳ *What process allowed this?* The local and CI gate optimized for "compile the app target" and did not model macOS LaunchServices as part of the test surface.

**Why 3**: Signing/notarization was split across several places: dev app install, Xcode project generation, screenshot scripts, CI, release packaging. Only some paths had stable signing comments, and release packaging still allowed ad-hoc fallback.
↳ *What assumption was wrong?* "This is only local/dev signing" was treated as harmless, even though macOS security identity is part of whether the app can launch without user interruption.

**Why 4**: The project had no executable policy that user-facing artifacts must be signed/notarized and test runners must be locally launchable. The policy lived in comments and tribal memory, not tests.
↳ *Why was that assumption encoded?* The gate checked functional correctness and compileability; UX interruption from OS security dialogs was not represented as a failing invariant.

**Why 5 (Root)**: Operational UX was not promoted to a release invariant. Tooling could pass while creating artifacts that macOS would distrust or that would prompt for credentials.

## Unanswered Whys

| Branch Point | Unexplored Question | Priority |
|--------------|---------------------|----------|
| Why 1 | Should old `Concerto`/`Maestro` DerivedData be cleaned by a one-time migration command? | Medium |
| Why 2 | Should `lf gate` run a lightweight `codesign --verify` probe on generated macOS runners when Xcode UI tests are selected? | High |
| Why 3 | Should all Xcode invocation settings move behind one shared helper instead of duplicated command arrays? | Medium |
| Release | Can the self-hosted release host run a patch-release dry run from this branch before land? Current sandbox cannot SSH to `mini-heart`. | Medium |

## Fixes

| Level | Fix | Prevents |
|-------|-----|----------|
| Immediate | Use ad-hoc local signing for macOS Xcode test runners instead of disabling signing. | Damaged `*-UITests-Runner` popups during gate/screenshot runs |
| Immediate | Make `ConnectionSecretStore` Keychain operations non-interactive. | Password sheets during app launch/test startup |
| Structural | Use repo-local DerivedData and disable automatic package resolution in wrapper-driven Xcode runs. | Stale renamed runners and surprise network updates during gate |
| Structural | Copy `swift/Package.resolved` into the generated Xcode workspace after `xcodegen`. | Offline Xcode runs falling back to network resolution |
| Structural | Refuse to build user DMGs without Developer ID signing and notarization credentials. | Shipped artifacts that trigger Gatekeeper for users |
| Systemic | Add regression tests that reject disabled macOS signing and skipped release notarization. | Process drift reintroducing the same popups |
| Systemic | Update `TESTING.md`, `swift/README.md`, and CI to show the signed command only. | Copy-paste recurrence |

## Changes to Implement

- [x] Replace `CODE_SIGNING_ALLOWED=NO` for macOS Loopflow UI test paths with explicit ad-hoc signing.
- [x] Add generated Xcode workspace `Package.resolved` copy so package resolution can be disabled.
- [x] Make Keychain token operations fail closed instead of prompting.
- [x] Fail release DMG builds when Developer ID signing or notarization is unavailable.
- [x] Add regression tests for signing and release hardening.
- [ ] Run the full Xcode UI test on an unsandboxed macOS host.
- [ ] Run `lf op release run patch` on `mini-heart` or another host with release credentials.
