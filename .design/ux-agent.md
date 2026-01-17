# Context Preview Panel

Make assembled context visible before running a task.

## What to build

A collapsible panel in the prompt launcher that shows exactly what will be sent to the agent—file names, content previews, and token counts per section.

## User story

> "I toggled 'Files' on but I have no idea what files are included. The token count jumped from 2k to 14k but I can't see why."

After: User clicks the token count or an expand arrow. Panel slides open showing:
- Docs section: README.md (1.2k), STYLE.md (0.8k)
- Files section: src/auth.py (3.4k), src/models.py (2.1k)
- Diff section: 47 lines changed (0.5k)
- Clipboard: 12 lines (0.3k)

User can click any item to preview its content. User can remove individual files by clicking ✕.

## Data structures

```swift
struct ContextPreview {
    let sections: [ContextSection]
    let totalTokens: Int
}

struct ContextSection {
    let kind: ContextKind  // .docs, .files, .diff, .clipboard, .attached
    let items: [ContextItem]
    var tokens: Int { items.reduce(0) { $0 + $1.tokens } }
    var isEnabled: Bool
}

struct ContextItem {
    let name: String           // "README.md" or "47 lines changed"
    let preview: String?       // first ~200 chars for hover/click preview
    let tokens: Int
    let path: String?          // for files, enables removal
}

enum ContextKind: String {
    case docs, files, diff, clipboard, attached

    var color: Color {
        switch self {
        case .docs: return .blue
        case .files: return .teal
        case .diff: return .green
        case .clipboard: return .purple
        case .attached: return .orange
        }
    }

    var icon: String {
        switch self {
        case .docs: return "doc.text"
        case .files: return "doc.on.doc"
        case .diff: return "plus.forwardslash.minus"
        case .clipboard: return "doc.on.clipboard"
        case .attached: return "paperclip"
        }
    }
}
```

## Key functions

```swift
// In AppState or a new ContextService

func assembleContextPreview() async -> ContextPreview
/// Builds preview by reading the same sources as buildCommand().
/// Returns structured data for UI display.

func removeContextItem(_ item: ContextItem, from section: ContextSection)
/// Removes a specific file from context. Only valid for .files and .attached.

func copyAssembledContext() -> String
/// Returns the full prompt text that would be sent to the agent.
/// Equivalent to CLI's `-c` flag.
```

```swift
// In PromptLauncher.swift

@State private var contextPreview: ContextPreview?
@State private var isPreviewExpanded: Bool = false
@State private var selectedPreviewItem: ContextItem?

var contextPreviewPanel: some View
/// Collapsible panel showing sections and items.
/// Each section has a header with icon, name, and token count.
/// Items within sections are clickable for preview.
```

## UI changes

### Token count becomes interactive

Current:
```
[ 14.2k ]  ← static number
```

After:
```
[ ▾ 14.2k ]  ← clickable, expands panel below
```

Or as a segmented bar (Cursor-style):
```
[████████░░░░░░░░] 14.2k
 docs  files diff
```

Clicking the bar or the number expands the preview panel.

### Preview panel (when expanded)

```
┌─────────────────────────────────────────────────┐
│ Context Preview                           [Copy]│
├─────────────────────────────────────────────────┤
│ ▼ Docs (2.0k)                                   │
│   📄 README.md                           1.2k   │
│   📄 STYLE.md                            0.8k   │
│                                                 │
│ ▼ Files (5.5k)                                  │
│   📄 src/auth.py                    ✕    3.4k   │
│   📄 src/models.py                  ✕    2.1k   │
│                                                 │
│ ▸ Diff (0.5k)                                   │
│                                                 │
│ ▸ Clipboard (0.3k)                              │
└─────────────────────────────────────────────────┘
```

- Sections are collapsible (▼/▸)
- Items in Files/Attached sections have ✕ to remove
- Hover on item shows preview popover
- Click on item shows full content in sheet
- [Copy] button copies full assembled prompt

### Integration with existing context bar

The existing context toggles (Docs, Files, Diff, Clipboard) remain. They're the "coarse" control. The preview panel is the "fine" control—see exactly what's included, remove specific items.

When a toggle is off, that section appears grayed out in the preview (if expanded).

## Constraints

- **Preview must match reality**: The preview must show exactly what `buildCommand()` will send. Same logic, same truncation.
- **Performance**: Don't re-read files on every keystroke. Cache the preview, invalidate when toggles change or files are added/removed.
- **Token estimation**: Use the same estimation as current `estimateTokens()`. Accuracy isn't critical—users want relative sizes, not exact counts.

## Done when

1. Token count is clickable and expands a preview panel
2. Preview shows sections with item-level breakdown
3. Files and attached items can be removed from preview
4. Copy button exports full assembled context
5. Toggling context options updates preview in real-time

**Verification:**
```bash
# Build and run Maestro
cd Maestro && xcodebuild -scheme Maestro -configuration Debug

# Manual test:
# 1. Open a repo with .lf/ config
# 2. Toggle Files on, click token count
# 3. Preview panel shows files touched by branch
# 4. Click ✕ on a file, token count decreases
# 5. Click Copy, paste into editor, verify content matches
```

## Out of scope

- @ mentions in prompt text (separate feature)
- Drag to reorder context priority
- Token limit warnings
- Minimap visualization

These are good ideas but each deserves its own design pass.
