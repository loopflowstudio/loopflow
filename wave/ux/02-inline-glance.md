# 02: Inline Glanceability

Inline the glance, link to the deep dive. Eliminate context-switches to GitHub/Cursor for the "check in" workflow.

Builds on session polish (shipped) — `CodeBlockView`, `CopyButton`, streaming cursor, and `Typography.code(12)` are established patterns. Reuse them here.

## What to build

Five features that make diffs and wave content visible inside Concerto.

### File items show diffs (G1)

`FileEdit` already has `path`, `kind`, and `diff` fields. `SessionState.projectCard()` drops the diff — it only renders paths as a label. Change file items to expand with colored diff lines.

New `DiffLinesView`: parse unified diff, green for `+`, red for `-`, muted for context and `@@` headers. `Typography.code(12)`, horizontal scroll, copy button.

### Wave diff stat per-file expand (G2)

Each file line in `diffStatSection` becomes tappable. Tap → expand to show that file's full diff inline (reuses `DiffLinesView`).

Requires lfd: lazy-load endpoint `GET /waves/:id/diff/:path` serving per-file unified diff. Don't send full diffs in every wave poll.

### Roadmap item expansion (G3)

Tapping a roadmap item expands to show first ~20 lines of its markdown. "Open in Cursor" link at bottom.

Add `content: String?` to `RoadmapItem`. `WaveContentParser` already reads these files — include truncated raw content.

### Wave README full content (G4)

Add "Show more" toggle to vision/goals/risks sections. Default: condensed bullet view (as today). Expanded: full section text with basic markdown rendering.

### Scratch doc glance (G5)

If `scratch/<branch>.md` exists in the worktree, show a "Design" section in Current tab. Condensed: first 5 lines. Expandable: full content with markdown rendering. "Open in Cursor" link.

## Constraints

- Inline views are for glancing. Deep dives link to GitHub (diffs/PRs) or Cursor (editing/docs).
- `DiffLinesView` is a shared component, reusable for G1, G2, and future features.
- No syntax highlighting in diffs — colored +/- lines only.

## Validation

- `swift test --package-path swift`
- `cargo test --all` (if lfd endpoint added)
- Manual: expand a file item to see diff, tap diff stat file to see per-file diff, expand roadmap item to read content

## Done when

You can check "what changed?" and "what's the plan?" without leaving Concerto.
