# Button Hierarchy

Add `GhostButtonStyle` and `DestructiveButtonStyle` to `DesignSystem.swift`. Restyle buttons by intent.

## Problem

All action buttons use `DarkButtonStyle` (filled burgundy). Stop and Archive look identical to Land and Next. No visual hierarchy signals which action is primary, secondary, or destructive.

## Three styles

| Style | Appearance | Usage |
|-------|-----------|-------|
| `DarkButtonStyle` (existing) | Filled burgundy, cream text | Primary: Land, Next, Retry, Run |
| `GhostButtonStyle` (new) | Text-only with accent color, hover surface | Secondary: View PR, Warp, Cursor, Clone |
| `DestructiveButtonStyle` (new) | Outline with statusError color | Destructive: Stop, Archive, Cancel |

## Implementation

### GhostButtonStyle (~20 lines)

```swift
struct GhostButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(palette.accent)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .fill(configuration.isPressed ? palette.surfaceMuted : Color.clear)
            )
    }
}
```

### DestructiveButtonStyle (~20 lines)

```swift
struct DestructiveButtonStyle: ButtonStyle {
    @Environment(\.palette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Typography.body())
            .foregroundStyle(Color.statusError)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(
                RoundedRectangle(cornerRadius: CornerRadius.md)
                    .strokeBorder(Color.statusError.opacity(configuration.isPressed ? 0.8 : 0.5), lineWidth: 1)
            )
    }
}
```

## Button restyle map

### DarkButtonStyle (primary) — keep as-is

| File | Button |
|------|--------|
| `WaveDetailPanel.swift` | Land |
| `WaveDetailPanel.swift` | Next |
| `WaveDetailPanel.swift` | Retry |

### GhostButtonStyle (secondary) — change from DarkButtonStyle

| File | Button |
|------|--------|
| `WaveDetailPanel.swift` | View PR |
| `WaveDetailPanel.swift` | Warp |
| `WaveDetailPanel.swift` | Cursor |

### DestructiveButtonStyle — change from DarkButtonStyle

| File | Button |
|------|--------|
| `WaveDetailPanel.swift` | Stop |
| `NextActionsBar.swift` | Archive |
| `InteractiveSessionView.swift` | Cancel |

## Scope

~60 lines total:
- ~40 lines: two new ButtonStyle structs in DesignSystem.swift
- ~20 lines: change `.buttonStyle(DarkButtonStyle())` to `.buttonStyle(GhostButtonStyle())` or `.buttonStyle(DestructiveButtonStyle())` in 6 places across 3 files

Files touched: `DesignSystem.swift`, `WaveDetailPanel.swift`, `NextActionsBar.swift`, `InteractiveSessionView.swift`
