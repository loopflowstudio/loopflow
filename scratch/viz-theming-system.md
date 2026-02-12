# Theming system for fast iteration

## Problem

Changing Concerto's visual design requires modifying hex values, rebuilding, and comparing before/after manually. There's no way to see multiple color approaches side-by-side. Phase 4 polish work will need dozens of visual tweaks — each one is a rebuild-and-squint cycle.

The palette infrastructure is solid (`LoopflowPalette`, `StatusColors`, `DesignSystem.swift`). The problem is plumbing: 17 views each call `LoopflowPalette.make(for: colorScheme)` independently. Swapping to a different palette means touching all 17.

## Approach

Environment-based palette injection. One `EnvironmentKey`, one injection point at the app root, 17 views simplified to `@Environment(\.palette)`. Theme variants are just different `LoopflowPalette` instances — swap one `.environment()` call and every view updates.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Singleton / global palette | Simple, no environment plumbing | Can't render two themes simultaneously in previews. Defeats the fast-comparison goal. |
| Full theme object (palette + typography + spacing) | More comprehensive | Over-scoped. Typography and spacing don't vary between themes — only colors do. Add later if needed. |
| SwiftUI `Color` asset catalog with named colors | Apple's recommended approach for theming | Asset catalogs don't support runtime variant switching or multi-theme previews. Good for light/dark, bad for A/B comparison. |
| `@Observable` theme manager | Reactive, could support runtime switching | Heavier than needed. Environment injection is the SwiftUI-native pattern. `@Observable` adds overhead for something that changes rarely (only at preview or app launch). |

## Key decisions

**Palette only, not full theme.** Typography, spacing, and corner radii are invariant across themes. Only colors change. Keeping the scope to `LoopflowPalette` means no new types, no new abstractions.

**Three palette variants: light, dark, deep wine.** Light and dark are production. Deep wine explores whether burgundy can go moodier — darker background (`#1E1215`), richer accent (`#8B2252`), warmer surface (`#2A1A20`). If it works, it validates the brand direction. If not, we learn the limits.

**Environment key on `LoopflowPalette` directly, not a wrapper.** Follows the existing `ScreenshotTabKey` pattern. No `ThemeProvider`, no `ThemeContext`, no indirection.

**`ThemePreview` renders light + dark + deep wine side-by-side.** Three columns in Xcode previews for instant visual comparison.

**Migrate all 17 views in one pass.** Doing it incrementally creates a half-migrated codebase. The migration is mechanical — delete `@Environment(\.colorScheme)` + computed palette, add `@Environment(\.palette)`.

**Inline palette hex values.** Compress pass removed intermediate named colors (`loopflowCreamElevated`, `loopflowCreamMuted`, etc.) that added indirection without value. Hex values are now directly in the palette definitions.
