## Try it!

```bash
swift test --package-path swift
cd swift
xcodegen generate
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Open Concerto, choose a repo from the portfolio window, and the repo window
should show only waves for that repo. Each row shows the wave name, repo chip,
and rollup status. No create-wave, wave detail, or workspace flow is exposed in
this slice.

The full Concerto UI command was also attempted:

```bash
xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

It generated and built the project, then failed because the UI test runner could
not initialize in this headless session: `Authentication canceled. Canceled by
user.`

## Intent

Start the outward wave model from blankness. This slice strips the repo window
down to a repo-filtered wave list so the app can grow from wave identity outward
instead of continuing to expose the old sidebar/detail/session workspace as the
default surface.

## Assumptions

- Today's `Wave.repo` remains the only repo source for this slice.
- A wave's future repo set is stubbed as `[wave.repo]` until the
  `Wave`/`RepoWork` DTO split lands.
- Repo windows should be display-only for now; selection and workspace flows are
  later slices.

## Key decisions

- Kept filtering as a simple normalized path comparison:
  `wave.repo.normalizedFilePath == currentRepoPath`.
- Added a repo chip derived from the repo directory name to match the slice-1
  row contract without changing wire models.
- Updated `swift/README.md` to describe the repo wave list and mark the
  multiplexer workspace as no longer exposed by default.

## Not included

- Create-wave flow.
- Open-wave detail or per-repo streams.
- Multiplexer/workspace reintroduction.
- `Wave`/`RepoWork` wire-model changes.
