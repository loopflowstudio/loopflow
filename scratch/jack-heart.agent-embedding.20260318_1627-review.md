# Gate review: jack-heart.agent-embedding.20260318_1627

## What was implemented

This branch turns the daemon and Concerto into one shared wave workspace instead of a list view plus ad-hoc terminal handoffs.

- `lfd` now persists `terminal_sessions`, exposes terminal-session HTTP routes, and emits richer waiting/attention state for interactive work.
- Concerto now treats a wave as a workspace: runs, attention, and tracked terminal sessions live together in the detail view, and terminal tabs can be restored after refresh.
- Attention handling was collapsed to the two shipped concepts (`interactive` and `algedonic`) so daemon types, HTTP DTOs, Python client code, Swift models, and UI all agree.
- The generated Xcode project now uses macOS app runpaths that point at `Contents/Frameworks`, fixing locally built Concerto app launches outside the package-test harness.

## Key choices

- **Persist terminal sessions in `lfd`, not in Swift-only UI state.** That makes session identity durable and gives every client one source of truth.
- **Keep the workspace view additive.** Embedded terminals live beside runs and attention instead of replacing the native work surface.
- **Collapse attention kinds at the API boundary.** The product ships two human-facing attention modes, so the contract now matches that reality instead of preserving extra taxonomy.
- **Fix framework lookup in `swift/project.yml`.** The runpath change lives in generated-project source, not as a post-build patch.

## How it fits together

`lfd` now owns long-lived terminal-session records alongside wave state. The HTTP layer exposes list/get/attach/start/complete/cancel operations, and Concerto consumes that contract through `LocalWaveService`, `LocalEventService`, `RepoState`, and `TerminalWorkspaceStore` to render a durable wave workspace.

Attention and waiting state flow through the same path: daemon types and DTOs normalize the payloads, then Swift models and stores use them to focus the right wave or terminal session.

## Risks and bottlenecks

- **Concerto UI automation is still the weakest gate.** On March 19, 2026, a clean `xcodebuild test -scheme Concerto` run still ends with `ConcertoUITests-Runner ... signal kill before establishing connection` after the app and unit/package tests finish.
- **Terminal-session recovery depends on daemon/store coherence.** Regressions here will show up as stale sessions, wrong selection, or mismatched wave focus rather than obvious crashes.
- **Attention-kind collapse is cross-surface.** Any stale caller still expecting older kind strings will break at the client boundary unless it uses the compatibility mapping.

## What's not included

- Remote terminal transport beyond the local daemon-backed session model.
- A deeper portfolio/calibration redesign beyond the workspace and attention surfaces already in this branch.
- A root-cause fix for the remaining local UI-test-runner bootstrap kill.

## Validation

There was no `scratch/jack-heart.agent-embedding.20260318_1627.md` design doc on this branch, so validation followed `TESTING.md`, `swift/README.md`, and the branch wave docs.

Passed:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`
- direct launch of the built macOS app after the runpath fix:
  - `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-fhniguselvodfhbnlvqizehygmun/Build/Products/Debug/Concerto.app/Contents/MacOS/Concerto -ui-test-mode mock-waves`

Still failing locally after a clean DerivedData rebuild:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - app build succeeds
  - unit/package tests inside the run pass
  - `ConcertoUITests-Runner` still exits during bootstrap before establishing the UI-test connection
