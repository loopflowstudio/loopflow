# Roadmap pane redesign

Minimal list + detail, driven by the README.

> "the README, reshaped to optimize for this usecase"

## What to build

Replace the current fat-card roadmap view with a tight list/detail pattern. The roadmap pane becomes a minimal list (title + priority per row). Detail lives in a separate multiplexer pane (`roadmapDetail`) showing the selected item's markdown. The wave README first paragraph becomes the sidebar tagline.

## Data structures

```swift
// Shared selection state — lives in environment, read by both panes
@Observable class RoadmapSelection {
    var selectedItemId: String?
    var selectedWaveId: String?
}

// New pane type added to PaneType enum
case roadmapDetail  // shows selected roadmap item's markdown + play button
```

`RoadmapItem`, `RoadmapPriority`, `WaveContent` — unchanged.

## Key functions

### RoadmapPaneView (list)

Rewrite `RoadmapPaneView` in `MultiplexerView.swift`. Strip it down to tight rows:

```swift
struct RoadmapRowView: View {
    let item: RoadmapItem
    let isSelected: Bool
    let isHovered: Bool
    let waveIsRunning: Bool

    var body: some View {
        HStack {
            Text(item.title)
                .foregroundStyle(item.isShipped ? palette.textSecondary : palette.text)
                .strikethrough(item.isShipped)

            Spacer()

            if isHovered && !item.isShipped {
                // play button (ingest & build)
            }

            priorityMenu(for: item)  // dropdown, same as today
        }
    }
}
```

- Title left, priority right, no badges/icons/content preview
- Shipped items: dimmed + strikethrough, sorted below unshipped
- Play button: appears on hover only, inline with the row
- Selected row: highlighted background, updates `RoadmapSelection.selectedItemId`
- Priority: inline dropdown menu on the priority text (same interaction as today)

### Keyboard navigation

The list pane captures focus and responds to:

| Key | Action |
|-----|--------|
| `j` / `↓` | Select next item |
| `k` / `↑` | Select previous item |
| `Enter` | Ingest & build selected item |

```swift
.onKeyPress(.downArrow) { selectNext(); return .handled }
.onKeyPress(.upArrow) { selectPrevious(); return .handled }
.onKeyPress(characters: "j") { selectNext(); return .handled }
.onKeyPress(characters: "k") { selectPrevious(); return .handled }
.onKeyPress(.return) { ingestSelected(); return .handled }
```

### RoadmapDetailPaneView (new)

New pane type in the multiplexer. Reads `RoadmapSelection` from environment:

- Full markdown rendering of the selected item's `.md` file
- Play button always visible (not hover-gated)
- Empty state when nothing selected: "Select a roadmap item"
- Updates reactively when selection changes

### README tagline extraction

`WaveContentParser` changes how it extracts the sidebar tagline:

- **New behavior:** first paragraph of the README (text before the first blank line, skipping any `#` heading) = `wave.visionTagline`
- **Backwards compat:** if the README has the old `## Vision` section and no leading paragraph, fall back to extracting from that section

### ReadmePaneView

Stays as-is for now. It shows the structured README sections. Can be revisited later — this change is about the roadmap, not killing the README pane.

## Constraints

- Pane communication goes through `RoadmapSelection` in the environment — multiplexer panes don't know about each other
- Keyboard navigation must not conflict with multiplexer-level shortcuts
- README parser must handle both old format (## Vision section) and new format (first paragraph) gracefully

## Done when

- `RoadmapPaneView` shows tight rows: title + priority, nothing else
- Play button appears on hover in list rows
- `j`/`k` and `↑`/`↓` navigate the list, `Enter` triggers ingest & build
- New `roadmapDetail` pane type exists in the multiplexer
- Selecting a roadmap item in the list updates the detail pane
- Detail pane renders full markdown with always-visible play button
- Sidebar tagline reads first paragraph from README
- Existing waves with old README format still render
