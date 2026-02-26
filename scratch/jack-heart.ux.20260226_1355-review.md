# Inline Glanceability G2–G5: Review Guide

## What was implemented

Four expand-on-tap features for WaveDetailPanel, each eliminating a context-switch to GitHub or Cursor:

- **G2** — Per-file diff expand in diff stat. New Rust endpoint `GET /waves/:id/diff?path=...` returns truncated unified diff. Swift taps file lines, lazy-loads diff, renders with `DiffLinesView`.
- **G3** — Roadmap item expansion. Parser extracts first 20 lines of markdown body (skipped for shipped items). Tap shows content + "Open in Cursor".
- **G4** — Show more/less on vision/goals/risks content cards. Full markdown rendering when expanded; 6-line bullet summary when collapsed.
- **G5** — Scratch doc section in Current tab. `WaveContentParser.parse()` gains `branch:` parameter to locate `scratch/<branch>.md`. Shows 5-line preview, expandable to full content.

## Key choices

| Decision | Why |
|----------|-----|
| Query param `?path=` for G2 endpoint | File paths contain slashes — path segments break |
| Truncate diffs at 500 lines | Glance, not review. GitHub link for full diffs |
| No content for shipped roadmap items | Nobody reads them post-ship. Keeps expand meaningful |
| Separate `expandedDiffFiles` state for G2 | G2 has async-loaded content + caching; G3/G4/G5 share `expandedSections` |
| `AttributedString(markdown:)` everywhere | Proven pattern from session view. Tables/images degrade gracefully |
| `branch` param on `WaveContentParser.parse()` | Scratch doc path depends on branch name. Clean threading through RepoState |

## How it fits together

```
WaveDetailPanel (UI)
├── contentCard() — G4 show more/less
├── roadmapItemRow() — G3 expand content
├── scratchDocSection — G5 design doc
└── diffStatFileLine() — G2 per-file diff
    └── loadFileDiff() → RepoState → LocalWaveService → lfd HTTP

WaveContentParser (data)
├── parseRoadmapItem() — extracts content + filePath (G3)
├── parseScratchDoc() — reads scratch/<branch>.md (G5)
└── contentCard() reads full text, display truncates (G4)

Rust (lfd)
└── get_wave_file_diff_handler → git_file_diff() (G2)
    └── validates path, truncates at 500 lines
```

## Risks and bottlenecks

- **Disk I/O on status change**: `loadWaveContent` parses from disk on every wave status transition. Fast for typical wave directories (few files), but could lag if someone has 50+ roadmap items.
- **Diff caching is view-local**: `@State fileDiffs` resets when WaveDetailPanel remounts (wave switch). Acceptable — diff data is cheap to re-fetch and stale diffs are worse than cache misses.
- **`extractFilePath` relies on git diff stat format**: Splits on `" | "` which matches standard git output. Files with `" | "` in their name would break — extremely unlikely in practice.

## What's not included

- Rust tests for the new endpoint (endpoint is thin — delegates to `git_file_diff` helper and existing `nearest_base_ref`)
- iOS views (LoopflowCore model changes are shared, iOS layout is separate)
- Syntax highlighting in diffs
- Remote repo diff stat (requires local worktree)

## Test coverage

- `WaveContentParserTests`: 4 test cases covering roadmap content extraction, shipped item exclusion, scratch doc with/without branch, scratch-doc-only content
- Swift: 200 tests pass
- Rust: clippy clean, fmt clean, all tests pass (1 pre-existing flaky test unrelated to this branch)

## Wave alignment

Advances wave goal: "Make diffs and wave content glanceable without leaving Concerto." All four features directly serve this — each removes one reason to context-switch.
