# Roadmap pane redesign review

## What was implemented

Added a list/detail roadmap workspace for Concerto.

- The roadmap pane is now a compact selectable list with title + priority, shipped items dimmed and sorted last, hover-to-play, and `j`/`k` + arrow-key navigation.
- Added a new `roadmapDetail` pane that renders the selected item's full markdown and keeps an always-visible ingest/build action next to the item metadata.
- Added shared `RoadmapSelection` state so the list and detail panes stay in sync without direct pane coupling.
- Updated the default multiplexer layout and pane palette/cycling so fresh workspaces open into roadmap list + roadmap detail + terminal.
- Changed README parsing so the wave tagline comes from the first paragraph when present, while still falling back to legacy `## Vision` content.
- Updated Swift docs and tests for the new layout and README parsing behavior.

## Key choices

- Kept cross-pane communication in a tiny environment-scoped `RoadmapSelection` object instead of teaching panes about each other. That keeps the multiplexer composition model intact.
- Reused the existing inline priority menu behavior so the redesign changes the information density, not the roadmap editing mechanics.
- Preserved `## Vision` parsing as a fallback because existing waves still depend on that structure, but prioritized the opening paragraph so newer READMEs can read more naturally.
- Swapped the default second pane from **Runs** to **Roadmap Detail** so the new pattern is visible immediately on new workspaces.

## How it fits together

`MultiplexerStore` now seeds workspaces with a roadmap list on the left and a vertical split of roadmap detail + terminal on the right. `RoadmapPaneView` owns list interactions and selection updates, while `RoadmapDetailPaneView` reacts to the shared selection and renders the chosen markdown file. `WaveContentParser` feeds both the sidebar tagline and roadmap rows by parsing README paragraphs plus the existing roadmap item files.

## Risks and bottlenecks

- The list/detail behavior is covered by model/parser tests, but there is still no dedicated UI test that clicks through roadmap selection and keyboard navigation.
- Full scheme `xcodebuild test` still fails in this environment because `ConcertoUITests-Runner` exits before bootstrapping the screenshot test; the app/unit test targets pass when run without the UI-test target.
- The roadmap list depends on hover for the inline play button, so reviewers should sanity-check that hover affordance feels obvious enough on a live build.

## What's not included

- No changes to the existing README pane beyond continuing to render the parsed sections.
- No new inline markdown editing or richer roadmap metadata in the list rows.
- No migration of persisted workspace layouts; existing saved layouts keep their current panes until reset.

## Done-when check

- Tight roadmap rows with title + priority only: implemented.
- Hover play button in list rows: implemented.
- `j`/`k`, arrows, and `Return` in the list: implemented.
- `roadmapDetail` pane type: implemented.
- Shared selection updates detail pane: implemented.
- Detail pane renders full markdown with visible play button: implemented.
- Sidebar tagline uses leading README paragraph with legacy fallback: implemented.
- Legacy README format still parses: covered by parser tests.

## Validation

Passed:

- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

Known failure to keep an eye on:

- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture`

Both fail here because `ConcertoUITests-Runner` exits early before the screenshot pipeline test can establish a connection.
