## Try it!

```bash
swift test --package-path swift
cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests
uv run python scripts/concerto-dev.py run-debug
```

In the app, open any wave with roadmap items and verify:

- the left pane is a compact roadmap list with title + priority only
- shipped items are dimmed, struck through, and sorted below planned items
- hovering a planned item reveals the inline play button
- `j`/`k`, `↑`/`↓`, and `Return` work in the roadmap list
- selecting a row updates **Roadmap Detail** with full markdown and an always-visible **Ingest & build** button
- the wave sidebar tagline comes from the README opening paragraph when present

Automated validation run on this branch:

- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

## Intent

Turn roadmap work into a scan-first workspace instead of a stack of expandable cards. The list stays lightweight for triage, the detail pane owns the full markdown, and wave taglines come from the README intro people actually read first.

## Assumptions

- The opening non-heading README paragraph is the best default source for a wave tagline when it exists.
- Roadmap items remain file-backed and keep using the current `ingestAndBuild` launch path for now.
- Live hover affordances still need a quick manual sanity check even though parser/layout/order behavior is covered by tests.

## Key decisions

- Added a dedicated `roadmapDetail` pane to the multiplexer default layout instead of keeping inline roadmap expansion.
- Shared selection between panes with a `RoadmapSelection` environment object so the panes stay loosely coupled.
- Preserved `## Vision` as a fallback when no opening README paragraph exists.
- Added a direct regression test for shipped-last ordering so the compact roadmap list behavior is pinned down in code.

## Not included

- Terminal tabs, direct tmux window launch from the play button, flow selection UI, and worker-capacity gate changes from the follow-up roadmap execution design.
- Dedicated UI automation for hover and keyboard interactions.
- Any change to roadmap priority buckets or the underlying ingest behavior.
