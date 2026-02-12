# Theming system for fast iteration

No way to quickly try different visual approaches and get feedback. Changing colors or spacing requires rebuilding and comparing manually.

## Context

Phase 2 findings inform the approach:

- **Token infrastructure is solid.** `LoopflowPalette`, `StatusColors`, `DesignSystem.swift` define all the right tokens. The problem is adoption (150+ system font calls, ~15 hardcoded radii) — but the tokens themselves are correct.
- **Linear's model**: 3 base variables (base, accent, contrast) generate an entire theme. Concerto's `LoopflowPalette.make(for:)` already does this for light/dark — extend it to support variant palettes.
- **SwiftUI previews are the fast feedback loop.** No rebuild needed — Xcode previews update live. Multiple preview variants (different themes) render side-by-side.

## Approach: Environment-based theme with preview variants

### 1. Make `LoopflowPalette` injectable via SwiftUI environment

Currently `LoopflowPalette.make(for: colorScheme)` is called in each view. Instead:

- Add `EnvironmentKey` for `LoopflowPalette`
- Inject it at the app root: `.environment(\.palette, LoopflowPalette.make(for: colorScheme))`
- Views read `@Environment(\.palette)` instead of calling `LoopflowPalette.make(for:)` directly
- Theme variants become: `.environment(\.palette, .warmBurgundy)` or `.environment(\.palette, .coolSlate)`

### 2. Define 2-3 palette variants for comparison

Beyond the existing light/dark:

- **Warm burgundy** (current default — cream/slate with burgundy accent)
- **Deep wine** (darker burgundy, more contrast, moodier)
- **Neutral** (gray accent instead of burgundy, for comparison baseline)

Each variant is just a `LoopflowPalette` instance with different hex values. No new types needed.

### 3. SwiftUI preview variants

Add a preview helper that renders a view in multiple themes side-by-side:

```swift
struct ThemePreview<Content: View>: View {
    let content: () -> Content
    var body: some View {
        HStack(spacing: 0) {
            content().environment(\.palette, .light)
            content().environment(\.palette, .dark)
            content().environment(\.palette, .deepWine)
        }
    }
}
```

### 4. Migrate views from `LoopflowPalette.make(for:)` to `@Environment(\.palette)`

This is the bulk of the work — find every `LoopflowPalette.make(for: colorScheme)` call and replace with the environment value. The API stays the same (`palette.background`, `palette.accent`, etc.), just the source changes.

## What this enables

- **Side-by-side theme comparison** in Xcode previews (no rebuild)
- **Quick color experiments**: duplicate a palette, change 3 values, preview
- **Foundation for user-facing theme selection** (future, not this PR)
- **Faster polish iteration**: Phase 4 work can preview changes across themes instantly

## What this doesn't do

- No user-facing theme picker (that's a future feature, not a fast-iteration tool)
- No runtime theme switching (previews are sufficient for fast feedback)
- No new design tokens — uses existing `LoopflowPalette` structure
- No font bundling (that's a separate Tier 2 polish item)

## Done when

- `LoopflowPalette` is injected via SwiftUI environment
- At least 2 palette variants exist for comparison
- A `ThemePreview` helper renders views in multiple themes
- Views use `@Environment(\.palette)` instead of `LoopflowPalette.make(for:)`
- A developer can evaluate a color change in under a minute using Xcode previews
