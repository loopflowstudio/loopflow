# Roadmap pane redesign — gate review

## What was implemented

- Split the default Concerto workspace into a compact **Roadmap** list, a new **Roadmap Detail** pane, and the existing terminal pane.
- Reworked roadmap rows to show just title + priority, sort shipped items below planned items, keep inline reprioritization, and support `j`/`k`, arrow keys, and `Return` for keyboard-first selection and launch.
- Added a shared `RoadmapSelection` state object so the list and detail panes stay in sync without tightly coupling the panes.
- Switched wave taglines to prefer the opening README paragraph, with `## Vision` as a fallback.
- Added parser/layout coverage plus a new regression test for the promised "planned items above shipped items" ordering.

## Key choices

- **List/detail over expandable cards.** The branch removes inline markdown previews from the list so scanning stays fast and the detail pane can own full markdown rendering.
- **Shared environment state over direct pane wiring.** `RoadmapSelection` is injected once from `MultiplexerView`, which keeps the pane tree composable and matches the existing environment-object pattern.
- **Opening paragraph as tagline.** The parser now treats the first non-heading paragraph as the wave tagline when present, which matches how README introductions are usually written, while still honoring existing `## Vision` content when no intro paragraph exists.
- **Ordering is tested explicitly.** `sortedRoadmapItems` is now visible to app tests so the shipped-last behavior has a direct regression test instead of living only in UI code.

## How it fits together

`MultiplexerStore` now seeds waves with a roadmap list on the left and a detail/terminal split on the right. `RoadmapPaneView` owns selection, hover, priority changes, and ingest triggers for the compact list, while `RoadmapDetailPaneView` renders the selected item's full markdown and repeats the ingest action in an always-visible spot. `WaveContentParser` supplies the richer README tagline text that the sidebar and workspace surfaces now consume.

## Risks and bottlenecks

- Hover affordance and keyboard navigation still rely on live macOS behavior; there is model/parser coverage, but no dedicated UI automation for hover/selection sync yet.
- The full `xcodebuild test -scheme Concerto` suite still has the pre-existing `ConcertoUITests-Runner` screenshot-pipeline startup failure, so validation remains targeted at `-only-testing:ConcertoTests`.
- The roadmap detail pane still launches via the existing `repoState.ingestAndBuild` HTTP path; the follow-up tmux-tab/direct-execution work is tracked separately in `scratch/terminal-tabs-and-flow-execution.md`.

## What's not included

- No terminal-tab work, direct tmux window launching, flow selector pane, or worker-capacity gate changes.
- No new UI screenshot/interaction test for hover or key navigation beyond the added unit-level ordering/layout/parser coverage.
- No change to roadmap priority semantics or file-backed ingest behavior.

## Validation

### Automated

- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

### Manual follow-up for reviewers

Run `uv run python scripts/concerto-dev.py run-debug`, open a wave with roadmap items, and verify:

- roadmap rows are compact and shipped items stay dimmed/struck through at the bottom
- hovering a planned item reveals the inline play button
- `j`/`k`, arrow keys, and `Return` work in the roadmap list
- selecting a row updates **Roadmap Detail** and keeps **Ingest & build** visible
- the sidebar tagline matches the README opening paragraph when present
