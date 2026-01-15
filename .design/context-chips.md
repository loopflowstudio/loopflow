# Always-visible context chips

## What to build

Replace the hidden context panel with an always-visible row of toggleable chips showing what's included in the prompt.

## The problem

Context options are hidden behind a "Context ▼" button. Users forget to check it, can't see what's active at a glance, and don't know the token cost until they expand the panel.

## Design references

- **Linear filter bar**: Toggleable chips that show active/inactive state, compact horizontal layout
- **Notion property pills**: Clean rounded chips with subtle backgrounds, easy scanning
- **Cursor context pills**: The `@file` mentions that appear as removable chips in the input area
- **Figma toolbar**: Always-visible toggles, no hidden panels for primary controls

## Design

A single horizontal row below the main input, always visible:

```
┌─────────────────────────────────────────────────────────────┐
│  [Docs ✓]  [Files ✓]  [Diff]  [Clipboard]   +2 files  14.2k │
└─────────────────────────────────────────────────────────────┘
```

- **Chips are toggleable**: Like Linear's filter chips. Active = filled with accent color, inactive = ghost/outlined.
- **Token count always visible**: Right-aligned, muted text. Like Notion's word count.
- **"+N files" badge**: Click opens file picker popover (like Linear's "+ Add filter").

When files are attached, they appear as removable chips (like Cursor's @-mentioned files):

```
[Docs ✓] [Files ✓] [Diff] [Clipboard]  ┃  src/auth.py ✕  tests/ ✕  │ 18.4k
```

## Data structures

No new data structures. Uses existing `AppState` properties:
- `includeDocs`, `includeDiff`, `includeDiffFiles`, `includePaste`
- `attachedFiles: [URL]`
- `estimatedTokens: Int`

## Key changes

```swift
// Remove from PromptLauncher:
@State private var showContextOptions = false
private var contextOptionsSection  // delete entirely

// Replace optionsBar with:
private var contextBar: some View {
    HStack(spacing: 8) {
        // Toggleable chips
        ContextChip(label: "Docs", isOn: $appState.includeDocs)
        ContextChip(label: "Files", isOn: $appState.includeDiffFiles)
        ContextChip(label: "Diff", isOn: $appState.includeDiff)
        ContextChip(label: "Clipboard", isOn: $appState.includePaste)

        Divider().frame(height: 16)

        // Attached files
        ForEach(appState.attachedFiles) { file in
            FileChip(file: file, onRemove: { ... })
        }
        AddFileButton()

        Spacer()

        // Token count
        Text("\(formatTokens(appState.estimatedTokens))")
            .font(.caption)
            .foregroundStyle(.secondary)
    }
}
```

## UI changes

Delete:
- Collapsible context panel
- "Context ▼" button
- Token distribution bar chart (nice but not essential)

Add:
- `ContextChip` view - toggleable pill with label
- `FileChip` view - file name with X button
- `AddFileButton` - "+" that opens file picker popover

Keep:
- Auto/Interactive segmented control (moves above context bar)
- Drag-drop for files (drops add to attachedFiles)

## Constraints

- Must fit in one row on reasonable window widths
- If too many files attached, show "+N more" with popover
- Chips should have keyboard shortcuts (Cmd+1 through Cmd+4?)

## Done when

Launch Maestro, see context state at a glance without clicking anything. Toggle chips, see token count update live.
