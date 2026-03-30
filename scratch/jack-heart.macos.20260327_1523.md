# Roadmap pane redesign — validation

## Try it

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
- selecting a row updates the **Roadmap Detail** pane with full markdown and the always-visible **Ingest & build** button
- the wave sidebar tagline comes from the README opening paragraph when present
