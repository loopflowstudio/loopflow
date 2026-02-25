# Stage 01: Multiplatform Concerto — Current State

## Goal

Ship one `Concerto` target that works across macOS, iPhone, and iPad while keeping core mobile-relevant behavior reusable.

## Decisions to keep

- Keep a single app target/scheme; branch only at platform shell boundaries.
- Keep iOS remote-only in Stage 01 (no local repo/worktree flows).
- Use capability injection for platform-only behavior instead of spreading `#if` through shared code.
- Enforce multiplatform boundaries with `scripts/check_swift_multiplatform_boundaries.py`.

## Current baseline

- iOS + macOS builds are wired under one target.
- iOS shell exists (`MobileRootView`, connection setup, wave list/detail).
- macOS shell behavior remains intact (windowing, keyboard routing, Ghostty/local bootstrap).
- Shared design tokens moved into `LoopflowCore`.
- Shared state extraction landed in `swift/LoopflowCore/State/` (`RepoState`, stores, `OutputBuffer`, `ChatState`).

## Remaining work to fully close Stage 01

1. **Runtime validation gap (iOS)**
   - Build coverage is in place, but interactive end-to-end validation is still light.
   - Re-run full device flows: connection setup → connect to `lfd` → wave list/detail/output on iPhone and iPad.

2. **Environment-specific validation instability**
   - macOS `xcodebuild test` currently fails in this environment at `ConcertoUITests-Runner` startup.
   - `cargo test --all` fails docker-dependent tests when Docker socket is unavailable.

## Validation notes for this branch

- Passed: `swift test --package-path swift`
- Passed: `uv run python scripts/check_swift_multiplatform_boundaries.py`
- Passed on available simulators:
  - iPhone 17
  - iPad Pro 11-inch (M5)
- Stage doc targets (iPhone 16, iPad Pro 11-inch M4) were unavailable in this environment.

## Exit condition for follow-up PR

Stage 01 is fully closed when simulator/runtime validation is re-confirmed on iPhone+iPad flows without architecture gaps.
