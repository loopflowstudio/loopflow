# Concerto NUX: Design-First Onboarding (Current State)

## Goal

Make first-run Concerto UX design-first instead of configuration-first:
- remove dead schema UI paths
- replace "create wave by name" with a design prompt launcher
- surface wave vision/goals/risks/roadmap directly in the detail panel
- reduce sidebar clutter for new users

## What shipped on this branch

### 1) Wave schema cleanup (Swift)
- Deleted `swift/LoopflowCore/Models/WaveSchema.swift`
- Removed `listWaveSchemas` from `WaveServiceProtocol` and `LocalWaveService`
- Removed schema state/actions from `RepoState`
- Removed schema creation/instantiation UI from `WaveSidebar`

### 2) Start wave flow changed to design entry
- `StartWaveView` now takes a design prompt (`"Describe what you want to build..."`)
- Submit action launches `lf design -c '<prompt>'` via `TerminalLauncher.launchDesign(...)`
- No direct wave creation in UI; `lf design` owns wave creation
- Launch is local-repo-only; non-local repo targets show an error

### 3) Wave content model + parser
- Added `WaveContent` and `RoadmapItem` models
- Added `WaveContentParser` that reads:
  - `wave/<name>/README.md` sections: `## Vision`, `## Goals`, `## Risks`, `## Metrics`
  - `wave/<name>/NN-*.md` roadmap files
- Roadmap completion is detected by `## Shipped`

### 4) Wave detail panel now shows wave intent
- Vision appears as subtitle in header
- Goals shown while idle
- Risks shown during review-like steps
- Roadmap rendered with shipped/pending indicators
- Content is loaded on selection and refreshed on status/activation changes

### 5) Sidebar cleanup
- Single create action: "Start designing"
- Empty state updated to design-first copy
- Orphan worktrees moved behind collapsed `DisclosureGroup` (persisted)

## Behavior details to remember

- "Risks during review" is currently gated by `wave.activeRun?.currentStep` containing `"review"` (case-insensitive substring check).
- Wave content parsing is convention-based. Non-standard heading names are intentionally ignored.
- Content loading is on-demand and cached in `WaveViewModel`; there is no filesystem watcher.

## Known limitations and follow-ups

- Design still opens in external terminal (intentional interim).
- Inline design chat/session depends on wave item `04-agent-harness`.
- Content parsing is on main-actor paths today; very large docs could cause minor UI hitching.
- Remote repo targets intentionally skip local filesystem wave-content reads.

## Validation status

Validated in branch review with:
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`

All passed in the recorded review run.
