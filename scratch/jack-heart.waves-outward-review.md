# Gate Review: Waves One Level Out, Slice 1

## What Was Implemented

Repo windows now open to a minimal, repo-scoped wave list. `ContentView` was
reduced from the old sidebar/detail/workspace surface to a header plus the waves
touching the current repo, with each row showing the wave name, repo chip, and
rollup status.

The implementation intentionally keeps today's single-repo `Wave.repo` model.
The list stubs the future repo set by filtering on `wave.repo == currentRepo`,
using normalized file paths so equivalent local paths compare consistently.

## Key Choices

- Kept the model split out of scope. `Wave`/`RepoWork` remains deferred, so the
  UI uses `WaveViewModel.repo` as the one repo chip for now.
- Removed the exposed command palette, sidebar/detail, attention queue, and
  multiplexer path from `ContentView` rather than hiding them behind disabled
  branches. The old components still exist for later slices.
- Left rows non-interactive. Slice 1 is the blank list only; opening a wave and
  restoring the in-repo workspace are later slices.
- Updated the Swift README so reviewer-facing docs describe the repo wave list
  instead of the old queue/workspace entry flow.

## How It Fits Together

`ContentView` computes a current repo path from `RepoState.currentRepo` or the
selected repo target, filters `repoState.waves` through `repoFilteredWaves`, and
renders `RepoWaveRow` values. `RepoWaveRow` is deliberately display-only: status
comes from the existing `WaveStatus`/`WaveViewModel.statusText`, and the repo
chip is derived from the normalized repo path's directory name.

## Risks And Bottlenecks

- The view-level filter is partly redundant because `RepoState.applyConnectedSnapshot`
  already scopes snapshots by repo. Keeping both is harmless for this slice, but
  later multi-repo `Wave.repos` work should centralize this around the real
  `RepoWork` shape.
- The full Xcode UI-test command cannot be completed in this headless run mode.
  It generated and built the project, then failed when `ConcertoUITests-Runner`
  could not initialize for UI testing because authentication was canceled.
- No visual screenshot was captured because this run has no rendering
  environment. The accessible row labels and unit coverage guard the display
  contract available from code.

## What's Not Included

- Create-wave flow.
- Opening a wave or selecting a per-repo stream.
- Reintroducing the multiplexer/workspace under a selected repo.
- The `Wave`/`RepoWork` DTO split across Rust, Python, Swift, and fixtures.
- Multi-user or cloning behavior.

## Validation

Passed:

- `swift test --package-path swift`
- `xcodegen generate`
- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

Attempted:

- `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

  Result: project generation/build succeeded, then the UI runner failed to
  initialize for UI testing with `Authentication canceled. Canceled by user.`
  The non-UI Xcode unit target passed immediately afterward.
