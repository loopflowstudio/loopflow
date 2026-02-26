# 02: Inline Glanceability (G2–G5)

## Problem

Concerto shows wave status but forces context-switches to GitHub and Cursor for basic questions: "What changed in this file?" "What does this roadmap item say?" "What's the full vision?" "What's the design doc?"

These are glance operations — 5–30 seconds of reading. The overhead of switching apps kills the check-in workflow.

**Who benefits:** Developers and leads checking in on wave progress. They want answers in Concerto, not in three browser tabs.

**Why now:** G1 shipped inline diffs for session file items. The component (`DiffLinesView`) and pattern (expand-on-tap) are proven. Extending them to the wave detail view is natural.

## Approach

Four features, one shared pattern. Each makes something tappable that wasn't, revealing content inline with an optional "Open in Cursor" escape hatch.

### Shared conventions (not a shared component)

All four features use the same interaction: tap header → expand content → optional "Open in Cursor" link. No shared `ExpandableContentCard` view — each feature handles its own expand inline because the content types differ (diffs, markdown, bullet lists). The shared pieces are:

- `@State private var expandedSections: Set<String>` on `WaveDetailPanel`, keyed by section identifier. Single source of truth for G3/G4/G5 expand/collapse state.
- Chevron indicator convention (right → down) matching session file item pattern.
- Markdown rendering via `AttributedString(markdown:, options: .inlineOnlyPreservingWhitespace)` where applicable (G3/G4/G5).
- "Open in Cursor" reuses existing `openInIDE(path:)` on WaveDetailPanel.

---

### G2: Wave diff stat per-file expand

**Current:** `diffStatSection()` renders `wave.diffStat` as colored text — filenames with +/- bars. Static, non-interactive.

**Change:** Each file line becomes tappable. Tap → fetch per-file diff from lfd → show inline using `DiffLinesView`.

#### Rust: new endpoint

```
GET /waves/:wave_id/diff?path=src/foo.rs
```

Query param for path (not path segment — file paths contain slashes). Returns:

```json
{ "diff": "--- a/src/foo.rs\n+++ b/src/foo.rs\n@@...\n-old\n+new" }
```

Implementation in `routes/waves.rs`:
1. Reject paths containing `..` — return 400. Defense in depth; git only operates on tracked files, but fail fast with a clear error.
2. Resolve wave → worktree via `worktree_path()`
3. Compute base ref via `nearest_base_ref()` (same as `build_wave_dto`)
4. Run `git diff <base>..HEAD -- <path>` in the worktree via `Command::new("git").arg("diff").arg(...)` — pass the path as a separate arg, never interpolate into a shell string.
5. Return unified diff string, or empty string if file has no diff

Truncate at 500 lines — append `\n... (truncated, N more lines)` if exceeded. Glance, not review.

#### Swift: interactive diff stat

Parse `wave.diffStat` to extract file paths (split by ` | `, take first part, trim). Each file line wraps in a `Button` that:
1. Toggles the file path in `expandedDiffFiles: Set<String>`
2. On first expand, calls `GET /waves/:id/diff?path=...`
3. Caches the response in `@State private var fileDiffs: [String: String]`
4. Shows `DiffLinesView(diff:)` below the stat line

Add a chevron indicator (right → down) matching the session file item pattern from `TranscriptItemCardView`.

#### Files touched
- `rust/loopflow/src/lfd/http/routes/waves.rs` — new handler
- `rust/loopflow/src/lfd/http/routes/mod.rs` — new `git_file_diff()` helper, route registration
- `swift/LoopflowCore/Services/WaveServiceProtocol.swift` — new `fileDiff(waveId:path:)` method
- `swift/Concerto/Platform/macOS/Views/WaveDetailPanel.swift` — interactive diff stat

---

### G3: Roadmap item expansion

**Current:** Roadmap items show `✓ 01 · Title` as single lines. Tap does nothing.

**Change:** Tap → expand to show first paragraph of the item's markdown. "Open in Cursor" at bottom.

#### Model: add `content` to RoadmapItem

```swift
public struct RoadmapItem: Sendable, Identifiable, Equatable, Hashable {
    public var id: String
    public var number: Int
    public var title: String
    public var isShipped: Bool
    public var content: String?  // new: first ~20 lines of markdown body
    public var filePath: String? // new: absolute path for "Open in Cursor"
}
```

#### Parser: read content in `WaveContentParser.parseRoadmapItem()`

After extracting the title:
1. Drop lines up to and including the first `# ` heading
2. Take up to 20 lines of remaining content
3. Trim trailing whitespace
4. Store the file URL path for the "Open in Cursor" link

Don't include content for shipped items — nobody reads them.

#### UI: expandable roadmap items

Wrap each roadmap item row in a `Button`. On tap, toggle item ID in `expandedRoadmapItems: Set<String>`. When expanded:
- Show content as markdown (via `AttributedString`) below the title line
- "Open in Cursor" button at the bottom (reuse `openInIDE` from `WaveDetailPanel`)

#### Files touched
- `swift/LoopflowCore/Models/WaveContent.swift` — add fields
- `swift/LoopflowCore/Services/WaveContentParser.swift` — populate content + filePath
- `swift/Concerto/Platform/macOS/Views/WaveDetailPanel.swift` — expandable roadmap items
- `swift/ConcertoTests/WaveContentParserTests.swift` — test content parsing

---

### G4: Wave README full content

**Current:** `contentCard()` shows up to 6 bullet-stripped lines for vision/goals/risks. No way to see more.

**Change:** Add "Show more" toggle. Expanded view shows full section text with markdown rendering.

#### Implementation

`WaveContent` already stores full section text (e.g., `vision: String?`). The truncation happens at display time in `contentLines(from:)` via `.prefix(6)`.

Modify `contentCard()`:
1. Accept an `isExpanded` binding
2. When collapsed: current behavior (6-line bullet view)
3. When expanded: render full text with `AttributedString(markdown:)` preserving whitespace
4. Toggle button at bottom: "Show more" / "Show less" in caption style

Track state in `expandedSections: Set<String>` keyed by section name ("vision", "goals", "risks").

#### Files touched
- `swift/Concerto/Platform/macOS/Views/WaveDetailPanel.swift` — modify `contentCard()`

---

### G5: Scratch doc glance

**Current:** No visibility into `scratch/<branch>.md` from Concerto.

**Change:** If the scratch doc exists, show a "Design" section in the Current tab. Condensed: first 5 lines. Expandable: full content with markdown. "Open in Cursor" link.

#### Model: add `scratchDoc` to WaveContent

```swift
public struct WaveContent: Sendable, Equatable, Hashable {
    // ... existing fields ...
    public var scratchDoc: String?      // new: full scratch doc content
    public var scratchDocPath: String?  // new: absolute path for "Open in Cursor"
}
```

#### Parser: read scratch doc in `WaveContentParser.parse()`

The parser needs the branch name to locate `scratch/<branch>.md`. Add a `branch` parameter:

```swift
static func parse(repoRoot: URL, waveName: String, branch: String?) -> WaveContent?
```

If `branch` is non-nil, check for `scratch/<branch>.md` in `repoRoot`. If it exists, read and store the full content.

Callers: `RepoState.loadWaveContent()` passes `wave.branch` (available from `WaveViewModel`).

#### UI: Design section

New section in Current tab, placed between goals and StepRunner (above the fold — design context matters most when idle). Shows:
- "Design" label with `doc.text` icon
- Condensed: first 5 lines
- Expanded: full markdown content
- "Open in Cursor" button

#### Files touched
- `swift/LoopflowCore/Models/WaveContent.swift` — add fields
- `swift/LoopflowCore/Services/WaveContentParser.swift` — read scratch doc, add `branch` param
- `swift/LoopflowCore/State/RepoState.swift` — pass branch to parser
- `swift/Concerto/Platform/macOS/Views/WaveDetailPanel.swift` — new "Design" section
- `swift/ConcertoTests/WaveContentParserTests.swift` — test scratch doc parsing

---

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Send full diffs in every wave poll | Simpler Swift code | Wastes bandwidth, violates "lazy-load" constraint, diffs can be large |
| Run `git diff` from Swift (like WaveContentParser reads files) | No Rust endpoint needed | Only works for local repos, breaks remote mode |
| Full markdown renderer (WebView or cmark) | Richer rendering | Rabbit hole — `AttributedString` is good enough for glancing, complex rendering is a risk the wave doc calls out |
| Show all roadmap content eagerly | No expand/collapse needed | Clutters the view — most items are shipped and irrelevant |
| Separate "Documents" tab | Clean separation | Defeats "check-in" workflow — users want everything in Current tab |

## Key decisions

1. **Query param for file path** in G2 endpoint — file paths with slashes break as URL path segments. `?path=src/foo.rs` is clean and unambiguous.

2. **Truncate diffs at 500 lines** — glance, not review. Large diffs get `(truncated, N more lines)`. GitHub link for the full thing.

3. **No content for shipped roadmap items** — nobody reads them after they ship. Saves parsing time and keeps the expand interaction meaningful.

4. **`branch` parameter on WaveContentParser.parse()** — minor API change, but scratch doc location depends on branch name. The branch is already available in `WaveViewModel`.

5. **Single `expandedSections: Set<String>` state** — one source of truth for all expand/collapse in WaveDetailPanel. G2 uses a separate `expandedDiffFiles` set because its state includes async-loaded content, but G3/G4/G5 share the sections set.

6. **`AttributedString(markdown:)` for all expanded content** — proven in the session view (`AssistantTextSegment`). Tables and images won't render perfectly, but that's fine — "Open in Cursor" exists for deep dives.

## Scope

**In scope:**
- G2: Per-file diff endpoint, tappable diff stat, inline DiffLinesView
- G3: Roadmap item content, expandable preview, "Open in Cursor"
- G4: Show more/less toggle on vision/goals/risks
- G5: Scratch doc section in Current tab, expandable, "Open in Cursor"
- Tests for parser changes and new endpoint

**Out of scope:**
- Syntax highlighting in diffs (colored +/- only, per constraint)
- Full document viewer (search, scroll-to-line, page navigation)
- iOS-specific layout (LoopflowCore model changes are shared, but iOS views are separate)
- Code editing inside Concerto
- Diff stat for remote repos without local worktrees

## Implementation order

1. **G4** (smallest — modify `contentCard()` only, no model changes)
2. **G3** (model + parser + UI, but all Swift-side)
3. **G5** (model + parser + UI, similar to G3 but needs `branch` param threading)
4. **G2** (Rust endpoint + Swift API + UI — most complex, builds on all the UI patterns established in G3–G5)

## Done when

```bash
swift test --package-path swift          # Swift tests pass
cargo test --all                         # Rust tests pass (G2 endpoint)
cargo clippy -- -D warnings              # No warnings
```

Manual verification:
- Tap a file in diff stat → inline diff appears (G2)
- Tap a roadmap item → content preview + "Open in Cursor" (G3)
- Click "Show more" on vision section → full text renders (G4)
- See "Design" section when scratch doc exists → expand → full content (G5)

**Wave goals advanced:** "Make diffs and wave content glanceable without leaving Concerto." All four features serve this directly — each removes one reason to context-switch.
