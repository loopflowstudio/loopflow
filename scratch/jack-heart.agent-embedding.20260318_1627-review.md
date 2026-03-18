# Gate review: jack-heart.agent-embedding.20260318_1627

## What was implemented

This branch turns the daemon and Concerto into a shared terminal workspace instead of a collection of disconnected launch points.

- `lfd` now persists terminal sessions, exposes terminal-session HTTP routes, and emits richer waiting/attention state for interactive work.
- Concerto now treats a wave as a workspace: runs, attention, and terminal sessions share one detail surface, with terminal sessions recoverable across refreshes.
- Attention handling was tightened around the two shipped concepts (`interactive` and `algedonic`) so the daemon API, Swift models, and UI all speak the same language.
- Python client and docs were updated to match the new API shape.

Gate polish added one extra fix on top of the branch work: the generated Xcode project now uses macOS runpaths that point at `Contents/Frameworks`, so locally built Concerto apps can resolve bundled frameworks when launched outside the package-test harness.

## Key choices

- **Persist terminal sessions in `lfd` instead of keeping them UI-local.** This lets Concerto reconnect after refreshes and makes terminal state part of the daemon contract, not app memory.
- **Keep the wave detail view workspace-first.** Embedded terminals are additive; they do not replace runs, attention, or repo context.
- **Collapse attention kinds at the API boundary.** The branch ships two real user-facing attention modes, so the branch removes extra taxonomy instead of preserving more names than the product uses.
- **Fix the macOS app at the project layer.** The runpath issue was corrected in `swift/project.yml` so regenerated Xcode projects carry the fix, rather than patching built artifacts by hand.

## How it fits together

`lfd` now owns long-lived terminal-session records alongside wave state. The Swift app consumes those records through `LocalWaveService` and `LocalEventService`, stores them in `RepoState` / `TerminalWorkspaceStore`, and renders them in the new wave workspace views next to runs and attention items.

Attention and waiting state bridge the same path in parallel: daemon types and HTTP DTOs normalize the data, then Swift models and stores route the right wave or terminal session into focus.

## Risks and bottlenecks

- **Concerto UI automation is still the weakest validation point locally.** After the runpath fix, a built Concerto app launches cleanly from DerivedData, but `xcodebuild test -scheme Concerto` still ends with `ConcertoUITests-Runner ... Early unexpected exit ... signal kill before establishing connection` in this environment.
- **Terminal session recovery depends on daemon/store coherence.** Regressions here will show up as stale sessions, wrong selection, or mismatched wave focus rather than obvious crashes.
- **Attention-kind collapse is cross-surface.** Any stale caller still expecting older kind strings will fail at the client boundary unless it uses the provided compatibility mapping.

## What's not included

- Remote terminal transport beyond the local daemon-backed session model.
- A deeper calibration/portfolio redesign beyond the workspace and attention surfaces already in this branch.
- A root-cause fix for the remaining local UI-test-runner bootstrap kill; the gate work only fixed the macOS framework lookup bug that was independently reproducible on built apps.

## Validation

There was no `scratch/jack-heart.agent-embedding.20260318_1627.md` design doc on this branch, so validation followed `TESTING.md` and the branch wave docs.

Passed:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`
- direct launch of the built macOS app after the runpath fix:
  - `scratch/xcode-derived/Build/Products/Debug/Concerto.app/Contents/MacOS/Concerto -ui-test-mode mock-waves`

Still failing locally:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - unit/package tests pass inside the run
  - `ConcertoUITests-Runner` still exits during bootstrap before establishing the UI-test connection
