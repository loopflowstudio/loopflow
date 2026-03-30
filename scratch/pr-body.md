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
- selecting a row updates the new **Roadmap Detail** pane with full markdown and the always-visible **Ingest & build** button
- the wave sidebar tagline comes from the README opening paragraph when present

Notes:

- `xcodebuild test -scheme Concerto ...` still hits an existing `ConcertoUITests-Runner` bootstrap crash in this environment before the screenshot pipeline test starts.

## Intent

Replace the old fat-card roadmap presentation with a tighter list/detail workspace that matches how roadmap work is actually reviewed: scan priorities quickly, then read one item in full. The change also updates README tagline extraction so the wave sidebar can be driven by a natural opening paragraph instead of forcing authors into a dedicated `## Vision` section.

## Assumptions

- Fresh workspaces should bias toward roadmap triage, so the default second pane can move from **Runs** to **Roadmap Detail**.
- Existing README content that still uses `## Vision` must keep rendering.
- Persisted layouts are allowed to keep their current pane arrangement until a user resets or creates a new workspace.

## Key decisions

- Added a shared `RoadmapSelection` environment object instead of direct pane references so the multiplexer stays loosely coupled.
- Kept roadmap priority editing inline and unchanged while removing the extra summary/status chrome from list rows.
- Sorted shipped items to the bottom without changing the underlying roadmap file order inside each shipped/unshipped group.
- Parsed the README opening paragraph first, then fell back to legacy `## Vision` content only when no leading paragraph exists.

## Not included

- No redesign of the README pane itself.
- No new UI automation for roadmap keyboard navigation yet.
- No migration step for previously persisted multiplexer layouts.
