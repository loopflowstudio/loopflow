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

**Palette only, not full theme.** The ingested item suggested "no new design tokens — uses existing `LoopflowPalette` structure." This is right. Typography, spacing, and corner radii are invariant across themes. Only colors change. Keeping the scope to `LoopflowPalette` means no new types, no new abstractions.

**Three palette variants: light, dark, deep wine.** Light and dark are the existing production palettes. Deep wine is the experimental variant — darker burgundy backgrounds, higher contrast, moodier. A neutral (gray accent) variant was considered but dropped: it exists only to prove burgundy is better, which we already believe. Deep wine explores whether we can push burgundy *further*, which is a more useful design question.

**Deep wine specifics.** Darker background (`#1E1215` — near-black with wine undertone), richer accent (`#8B2252` — shifted toward magenta), warmer surface (`#2A1A20`). The goal: test whether Concerto can go moodier without losing legibility. If deep wine works, it validates the brand direction. If it doesn't, we learn where the limits are.

**Environment key on `LoopflowPalette` directly, not a wrapper.** The existing `ScreenshotTabKey` pattern in `ScreenshotWindow.swift` is the template. Simple struct, `EnvironmentValues` extension, done. No `ThemeProvider`, no `ThemeContext`, no indirection.

**`ThemePreview` renders light + dark + deep wine side-by-side.** Three columns, zero spacing. A developer previewing any view sees all three themes simultaneously. This follows the VISUAL_DESIGN.md principle of making comparison instant.

**Migrate all 17 views in one pass.** The migration is mechanical: delete `@Environment(\.colorScheme)` + computed `palette` property, add `@Environment(\.palette)`. Same variable name, same API surface. No behavior change. Doing it incrementally creates a half-migrated codebase where some views use environment and some use `make(for:)` — confusing and error-prone.

**`DarkButtonStyle` migrates too.** It currently calls `LoopflowPalette.make(for: colorScheme)` inside `makeBody`. After migration: `@Environment(\.palette)`. Button styles support environment reading.

**Root injection in `ConcertoApp.body`.** Each `WindowGroup` and `Window` already chains `.tint()` and `.preferredColorScheme()`. Adding `.environment(\.palette, LoopflowPalette.make(for: resolvedScheme))` fits the pattern. The resolved scheme comes from the existing `preferredScheme` logic.

**Palette must react to `colorScheme` changes.** When the user switches appearance (system/light/dark via the menu), the palette environment value must update. This happens naturally: `ConcertoApp.body` recomputes when `appearanceMode` changes, which recomputes `preferredScheme`, which recomputes the palette passed to `.environment()`. For the system appearance case, a `@Environment(\.colorScheme)` read in `ConcertoApp` triggers recomputation when the OS scheme changes.

## Scope

**In scope:**
- `PaletteKey` environment key + `EnvironmentValues.palette` extension (in `BrandColors.swift`)
- Three static palette variants: `.light`, `.dark`, `.deepWine` (on `LoopflowPalette`)
- `ThemePreview<Content>` helper view (new file: `Concerto/Views/ThemePreview.swift`)
- Migrate 17 views from `LoopflowPalette.make(for:)` to `@Environment(\.palette)`
- Migrate `DarkButtonStyle` in `DesignSystem.swift`
- Root injection in `ConcertoApp.body`
- Update 1-2 previews to use `ThemePreview` as demonstration

**Out of scope:**
- Runtime theme switching or user-facing theme picker
- Typography or spacing variants
- Font bundling
- Status color variants (status colors are semantic, not thematic)
- Migrating all previews to `ThemePreview` (only demo a couple; views that don't need theme comparison keep simple previews)

## Implementation plan

### 1. Add environment key and palette variants to `BrandColors.swift`

```swift
// Environment key for palette injection
struct PaletteKey: EnvironmentKey {
    static let defaultValue = LoopflowPalette.light
}

extension EnvironmentValues {
    var palette: LoopflowPalette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }
}
```

Add static properties to `LoopflowPalette`:

```swift
extension LoopflowPalette {
    static let light = LoopflowPalette(
        background: .loopflowCream,
        surface: .loopflowCreamElevated,
        surfaceMuted: .loopflowCreamMuted,
        border: Color(hex: 0xE3DDD5),
        text: .loopflowText,
        textSecondary: .loopflowTextSecondary,
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    static let dark = LoopflowPalette(
        background: .loopflowSlate,
        surface: .loopflowSlateElevated,
        surfaceMuted: .loopflowSlateMuted,
        border: Color(hex: 0x46505B),
        text: .loopflowTextLight,
        textSecondary: .loopflowTextSecondaryLight,
        accent: .loopflowBurgundy,
        accentHover: .loopflowBurgundyHover
    )

    static let deepWine = LoopflowPalette(
        background: Color(hex: 0x1E1215),
        surface: Color(hex: 0x2A1A20),
        surfaceMuted: Color(hex: 0x35222A),
        border: Color(hex: 0x4A3040),
        text: Color(hex: 0xF5EDE8),
        textSecondary: Color(hex: 0xC8B0A8),
        accent: Color(hex: 0x8B2252),
        accentHover: Color(hex: 0xA52D63)
    )

    static func make(for scheme: ColorScheme) -> LoopflowPalette {
        scheme == .dark ? .dark : .light
    }
}
```

### 2. Create `ThemePreview.swift`

```swift
struct ThemePreview<Content: View>: View {
    let content: () -> Content

    var body: some View {
        HStack(spacing: 0) {
            content()
                .environment(\.palette, .light)
                .environment(\.colorScheme, .light)
            content()
                .environment(\.palette, .dark)
                .environment(\.colorScheme, .dark)
            content()
                .environment(\.palette, .deepWine)
                .environment(\.colorScheme, .dark)
        }
    }
}
```

### 3. Inject palette at app root in `ConcertoApp.swift`

Add `@Environment(\.colorScheme) private var systemScheme` to `ConcertoApp`. Compute the resolved palette from `appearanceMode` and `systemScheme`. Apply `.environment(\.palette, resolvedPalette)` to each window group alongside the existing `.preferredColorScheme()`.

### 4. Migrate views (mechanical find-and-replace)

For each of the 17 view files:
- Delete `@Environment(\.colorScheme) private var colorScheme`
- Delete `private var palette: LoopflowPalette { LoopflowPalette.make(for: colorScheme) }` (or equivalent)
- Add `@Environment(\.palette) private var palette`

For `DarkButtonStyle` in `DesignSystem.swift`:
- Delete `@Environment(\.colorScheme) private var colorScheme`
- Delete `let palette = LoopflowPalette.make(for: colorScheme)`
- Add `@Environment(\.palette) private var palette`

### 5. Update a couple previews to demonstrate `ThemePreview`

Pick `WelcomeWindow` and `FlowProgressPills` (one simple, one data-rich) and wrap their previews in `ThemePreview`.

## Files changed

| File | Change |
|------|--------|
| `swift/Concerto/BrandColors.swift` | Add `PaletteKey`, `EnvironmentValues.palette`, static palette variants |
| `swift/Concerto/Views/ThemePreview.swift` | New: `ThemePreview<Content>` |
| `swift/Concerto/ConcertoApp.swift` | Add palette environment injection |
| `swift/Concerto/DesignSystem.swift` | Migrate `DarkButtonStyle` |
| 17 view files | Migrate from `make(for:)` to `@Environment(\.palette)` |

Total: ~20 files, ~3 net new lines per view migration (delete 2-3, add 1), plus ~60 lines for the environment key, variants, and `ThemePreview`.

## Done when

- `swift build --package-path swift` succeeds
- `swift test --package-path swift` passes
- No remaining calls to `LoopflowPalette.make(for: colorScheme)` in view files
- `ThemePreview` renders three columns in Xcode preview
- Deep wine palette displays a visually distinct, legible variant
