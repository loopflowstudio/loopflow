# Design Review: Concerto NUX (Design-First Onboarding)

## What was implemented

- Removed all Swift-side `WaveSchema` support that pointed at deleted backend APIs.
- Reworked empty-state creation flow from “create wave by name” to “start designing”:
  - `StartWaveView` now accepts a design prompt.
  - Submitting launches `lf design -c '<prompt>'` in terminal via `TerminalLauncher.launchDesign(...)`.
- Added wave content modeling and parsing:
  - New `WaveContent` / `RoadmapItem` model types.
  - New `WaveContentParser` for `wave/<name>/README.md` sections (`Vision`, `Goals`, `Risks`, `Metrics`) plus numbered roadmap files (`NN-*.md`) with shipped detection via `## Shipped`.
- Extended `WaveViewModel` and `RepoState` to cache and refresh parsed wave content.
- Updated `WaveDetailPanel` to surface wave context:
  - Vision in header subtitle.
  - Goals in idle state.
  - Risks during review steps.
  - Roadmap list with shipped/pending indicators.
- Simplified `WaveSidebar`:
  - Single create action (“Start designing”).
  - Orphan worktrees moved behind a collapsed disclosure by default.
  - Empty state updated to “No waves yet” + “Start designing”.
- Added parser coverage in `WaveContentParserTests`.

## Key choices

- **Terminal-first design launch now, inline session later:** launching `lf design` through terminal ships UX gains immediately without waiting on unified harness work.
- **Filesystem parser instead of API:** content is read from local `wave/` files on demand; this avoids backend changes and matches existing wave-config read patterns.
- **On-demand caching in `RepoState` / `WaveViewModel`:** avoids continuous file watching while keeping selected wave content fresh on status/selection changes.
- **`review` step detection by step-name substring:** lightweight implementation for showing risks during review phases.

## How it fits together

Sidebar “Start designing” clears wave selection, showing `StartWaveView`. Submitting a prompt launches `lf design`, which creates/updates wave files and wave state outside the app.

When a wave is loaded or changes state, `RepoState.loadWaveContent` parses wave docs and stores `WaveContent` on the wave view model. `WaveDetailPanel` renders that cached content contextually (vision/goals/risks/roadmap) alongside operational run controls.

## Risks and bottlenecks

- **Main-thread file parsing:** content parsing currently runs on the main actor path; very large wave docs could cause minor UI hitching.
- **Convention-driven parsing:** section detection is strict (`## Vision`, `## Goals`, `## Risks`, `## Metrics`), so non-standard headings are ignored.
- **Review-step heuristic:** showing risks depends on `currentStep` containing `"review"`; custom naming may miss the cue.
- **Local-only content reads:** remote repo targets intentionally skip content parsing.

## What's not included

- No inline agent chat/session harness for design yet (still terminal-based).
- No filesystem watcher for wave content (refresh is event/selection/status driven).
- No backend/API support for wave content.
- No changes to Rust/Python wave schema handling beyond removing dead Swift client references.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`

All passed locally.
