# G1: File Items Show Diffs

Expand file items inline to show colored diff lines. Foundation for all inline diff features.

**Source:** wave/ux/02-inline-glance.md (G1)

## Problem

`FileEdit` has `path`, `kind`, and `diff` fields, but `SessionState.projectCard()` sets `detail: nil` for file items — no expand chevron, no diff content. Users see "src/foo.swift, src/bar.swift" and nothing else. To check what an agent actually changed, they context-switch to GitHub or Cursor.

This blocks the "check in" workflow that wave 02 is about: "You can check 'what changed?' without leaving Concerto."

## Approach

Three changes, one new file:

### 1. New component: `DiffLinesView` (`swift/Concerto/Views/DiffLinesView.swift`)

A standalone SwiftUI view that takes a unified diff string and renders colored lines.

**Parsing model (`DiffLine`):**

```swift
struct DiffLine: Identifiable {
    let id: Int          // line index
    let text: String
    let kind: DiffLineKind
}

enum DiffLineKind {
    case addition    // line starts with "+" (not "+++")
    case deletion    // line starts with "-" (not "---")
    case hunk        // line starts with "@@"
    case header      // "---" or "+++" file header
    case context     // everything else
}
```

**Parsing:** Pure function `parseDiffLines(_ diff: String) -> [DiffLine]`. Line-by-line prefix matching — no regex, no dependency. Exposed as `internal` for testing.

**Rendering:**
- `Typography.code(12)` for all lines
- `Color.statusSuccess` for additions (green, `#2D6A4F`) — matches existing `coloredDiffStatLine` in WaveDetailPanel
- `Color.statusError` for deletions (orange, `#B45309`) — matches existing convention
- `palette.textSecondary` for context, hunk headers, and file headers
- `ScrollView(.horizontal)` with `.fixedSize(horizontal: true, vertical: false)` for wide lines
- `.textSelection(.enabled)` on the whole block
- Copy button using `copyToClipboard()` from PlatformHelpers, visible on hover (macOS) or always (compact). Same pattern as `CopyButton` in WaveSessionView but self-contained — DiffLinesView is reusable outside the transcript.
- `@Environment(\.accessibilityReduceMotion)` respected for any animations
- `accessibilityLabel("Diff: N additions, M deletions")` on the container

**Why self-contained copy button:** G2 will embed DiffLinesView inside WaveDetailPanel where there's no parent CopyButton. DiffLinesView must work standalone.

### 2. Wire diffs through `projectCard()` (`swift/LoopflowCore/State/SessionState.swift`)

```swift
case .file(let file):
    let paths = file.changes.map(\.path).filter { !$0.isEmpty }
    let diffs = file.changes.compactMap(\.diff).filter { !$0.isEmpty }
    let combinedDiff = diffs.joined(separator: "\n")
    return TranscriptItemCard(
        type: .file,
        label: paths.isEmpty ? "File change" : paths.joined(separator: ", "),
        status: file.status,
        detail: combinedDiff.isEmpty ? nil : combinedDiff
    )
```

Multiple FileEdits in one FileItem concatenate naturally — unified diff format already contains `--- a/path` / `+++ b/path` headers that visually separate files.

When no diffs are available (all `diff` fields nil), `detail` stays nil — no expand button, behaves exactly as today.

### 3. Route file detail through `DiffLinesView` (`swift/Concerto/Views/WaveSessionView.swift`)

In `TranscriptItemCardView`, when `card.type == .file` and expanded, render with `DiffLinesView` instead of plain monospace text:

```swift
if isExpanded, let detail = card.detail {
    if card.type == .file {
        DiffLinesView(diff: detail)
            .hoverTracking { hovering in isHoveringDetail = hovering }
    } else {
        // existing detail rendering (CopyButton + monospace/caption text)
    }
}
```

DiffLinesView handles its own copy button, so the parent's CopyButton is skipped for file items.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Structured `changes: [FileEdit]?` on `TranscriptItemCard` | Preserves per-file boundaries without re-parsing | Adds file-specific coupling to a generic card type. Type-specific rendering belongs in the view layer, not the card model. Unified diff format already separates files with headers. |
| WebView with diff2html | Syntax highlighting, familiar GitHub look | Heavyweight. Violates "no new dependencies." Overkill for colored +/- lines. |
| Per-file separate transcript cards | Each FileEdit gets its own expandable card | Requires restructuring transcript grouping. Agents send grouped file items — splitting loses context. More disruptive for one feature. |
| Reuse existing monospace detail path | Zero view changes — just pass diff as detail | No colored lines. Everything renders as flat `palette.textSecondary`. The whole point is green/red coloring. |

## Key decisions

**Concatenate, don't restructure.** Multiple file diffs join as one string rather than adding structured data to TranscriptItemCard. The unified diff format already contains file path headers (`---`/`+++`) that serve as visual separators. This keeps the generic card model clean and avoids coupling.

**Self-contained DiffLinesView.** Includes its own copy button rather than relying on the parent. G2 will embed this in WaveDetailPanel where there's no parent copy infrastructure. Reusability > deduplication.

**statusSuccess/statusError for diff colors.** Follows the existing convention in `WaveDetailPanel.coloredDiffStatLine()` where `+` is green (`statusSuccess`) and `-` is orange (`statusError`). Consistent within the app, even though traditional diffs use pure red. User muscle memory adapts to the app's palette.

**No truncation.** Show full diff content. "Inline views are for glancing" is about the interaction model (tap to expand, not forced on you), not about hiding content. If a diff is long, the user scrolls. Truncation adds complexity for questionable value — a user who expanded *wants* to see the diff. If this proves unwieldy with real sessions, truncation is a simple follow-up.

**Line-by-line prefix matching, not full unified diff parsing.** Checking if a line starts with `+`, `-`, `@@`, `---`, `+++` covers 100% of unified diff output. No regex, no hunk range parsing, no multi-line awareness needed. Simple, fast, correct.

## Scope

- **In scope:**
  - `DiffLinesView` as a standalone, reusable component
  - File item expand/collapse in session transcript
  - Colored diff lines (green additions, orange deletions, muted context)
  - Copy button on DiffLinesView
  - Tests for diff line parsing
  - Accessibility (labels, reduce motion)

- **Out of scope:**
  - Syntax highlighting within diff lines
  - Line numbers
  - Inline diff (word-level highlighting within changed lines)
  - G2 integration (wave diff stat expand) — separate item
  - Truncation / "show more" for long diffs
  - FileEdit `kind` badge (create/edit/delete indicator)

## File map

| File | Action | What |
|------|--------|------|
| `swift/Concerto/Views/DiffLinesView.swift` | Create | DiffLinesView component, DiffLine model, parseDiffLines() |
| `swift/LoopflowCore/State/SessionState.swift` | Modify | projectCard() passes concatenated diffs as detail |
| `swift/Concerto/Views/WaveSessionView.swift` | Modify | TranscriptItemCardView routes .file to DiffLinesView |
| `swift/ConcertoTests/DiffLinesViewTests.swift` | Create | Test parseDiffLines with additions, deletions, hunks, multi-file |

## Done when

- File items in session transcripts show an expand chevron when diff data exists
- Tapping the chevron reveals colored diff lines (green `+`, orange `-`, muted context/headers)
- Copy button on the diff copies raw unified diff text
- DiffLinesView is importable and usable standalone (ready for G2)
- `parseDiffLines` has tests covering: additions, deletions, context, hunk headers, file headers, empty input, multi-file diffs
- `swift test --package-path swift` passes
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` passes

**Wave goal advanced:** "You can check 'what changed?' without leaving Concerto." (02-inline-glance)
