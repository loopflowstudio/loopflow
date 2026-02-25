# Stage 01 Multiplatform Concerto — Review

## What was implemented

- Made the Swift package and Xcode project build for both macOS 15 and iOS 18 with one `Concerto` target.
- Added platform shell boundaries:
  - macOS shell keeps portfolio/repo windows, keyboard routing, Ghostty, and local bundled-daemon/bootstrap behavior.
  - iOS shell adds `MobileRootView` with iPhone (`TabView` + list/detail) and iPad (`NavigationSplitView`) navigation.
- Added iOS remote connection setup flow (`ConnectionSetupView`) with saved connection profiles (`MobileConnectionProfilesStore`) and remote repo selection.
- Added capability injection for local shell bootstrapping in `WaveService`/`RepoState` (`shellCommandRunner`, injected `startBundledDaemon`) so iOS is remote-only while macOS keeps local bootstrap.
- Moved shared design primitives to `LoopflowCore` (`BrandColors`, `DesignSystem`) and updated Concerto views to import shared tokens.
- Added multiplatform boundary guardrail script (`scripts/check_swift_multiplatform_boundaries.py`) and wired it into `scripts/validate_branch.py` + `TESTING.md`.

## Key choices

- **Capability injection over platform checks in core behavior**: local shell execution now comes from injected shell code (`LocalShellCommandRunner` on macOS, `nil` on iOS), avoiding hard macOS coupling in shared services.
- **Whole-file platform splits**: macOS-only files are wrapped or isolated under `Platform/macOS`, iOS-only views under `Platform/iOS`, minimizing inline `#if` churn.
- **One target/scheme strategy**: no separate mobile app target was introduced, preserving a single product entrypoint and shared state surface.
- **Boundary enforcement script**: added a lightweight static check to block macOS-only imports in `LoopflowCore` and non-shell `#if` additions.

## How it fits together

`ConcertoApp` now selects platform shells at app entry. Shared models/services/design tokens come from `LoopflowCore`, while platform-specific behavior is injected from shell files (`Platform/macOS`, `Platform/iOS`). `RepoState` remains the orchestrator, with connection/bootstrap behavior delegated through injected capabilities and platform convenience initializers.

## Risks and bottlenecks

- **Known architecture gap vs Stage 01 design**: `RepoState`/store classes remain under `swift/Concerto/State` (not yet moved to `LoopflowCore/State`). This is tracked in `scratch/questions.md`.
- **iOS runtime behavior still lightly validated**: builds pass for iPhone/iPad simulators, but interactive connection/wave flows were not fully exercised in this gate pass.
- **macOS UI test harness fragility in this environment**: `xcodebuild test` (full scheme) fails due `ConcertoUITests-Runner` early process exit; unit suites compile/pass.
- **Repo-wide test dependency on local Docker**: `cargo test --all` fails on two docker startup tests when `/var/run/docker.sock` is unavailable in environment.

## What's not included

- Stage 02 action buttons (`suggest_actions`) and Stage 03 multi-client continuity protocol work.
- Embedded terminal and command palette on iOS.
- Phone step runner/typeaheads, offline mode, and App Store packaging.
- Full state extraction of `RepoState`/stores into `LoopflowCore/State` (explicit follow-up).

## Wave alignment

- **Goal alignment**: this branch advances Stage 01 by shipping iOS shell wiring, multiplatform build gating, shared design extraction, and boundary guardrails.
- **Risk handling**: explicitly addresses `#if` sprawl/macOS import drift risk with script enforcement.
- **Observable validation from this gate pass**:
  - `swift test --package-path swift` ✅
  - `uv run python scripts/check_swift_multiplatform_boundaries.py` ✅
  - `xcodebuild ... -destination 'platform=iOS Simulator,name=iPhone 17' build` ✅
  - `xcodebuild ... -destination 'platform=iOS Simulator,name=iPad Pro 11-inch (M5)' build` ✅
  - `tests/e2e/test_smoke.sh` ✅
