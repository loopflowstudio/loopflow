# Roadmap pane redesign — validation

## Automated

- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

## Manual follow-up for reviewers

Run `uv run python scripts/concerto-dev.py run-debug`, open a wave with roadmap items, and verify:

- roadmap rows are compact and shipped items stay dimmed/struck through at the bottom
- hovering a planned item reveals the inline play button
- `j`/`k`, arrow keys, and `Return` work in the roadmap list
- selecting a row updates **Roadmap Detail** and keeps **Ingest & build** visible
- the sidebar tagline matches the README opening paragraph when present
