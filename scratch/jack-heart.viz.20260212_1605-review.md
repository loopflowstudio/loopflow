# Review: viz phases 1–4 — screenshots, design audit, theming, polish

## What was implemented

Unified theming system for Concerto (macOS app) aligned with VISUAL_DESIGN.md. The branch:

- **Bundled brand fonts** (Cormorant Garamond, Lato, JetBrains Mono) with runtime registration via CoreText
- **Introduced `LoopflowPalette`** — a theme struct with light, dark, and deep-dark variants, distributed through SwiftUI environment
- **Migrated all views** from `colorScheme`-based styling to palette-based tokens (background, surface, border, text, accent)
- **Added screenshot infrastructure** — expanded screenshots.yaml, new generate_screenshots.py, ScreenshotWindow with mock data support
- **Cleaned up design tokens** — removed unused Typography methods, added button style variants (Ghost, Destructive), used CornerRadius constants
- **Consolidated roadmap** — completed phases 1–6, collapsed into single remaining phase 7

## Key choices

**Font loading via `#if SWIFT_PACKAGE`**: SPM uses `Bundle.module` for copied resources, xcodegen uses `Bundle.main`. A compile-time branch selects the right bundle, avoiding runtime bundle-searching hacks.

**Palette as environment value, not `@Environment(\.colorScheme)`**: Views read `.palette` instead of deriving colors from colorScheme. This enables non-system themes (deep-dark) and centralizes color logic in `LoopflowPalette`.

**No Info.plist font registration**: Fonts are registered programmatically via `CTFontManagerRegisterFontsForURL` at app init. This avoids the brittleness of ATSApplicationFontsPath and works across both build systems.

**`type: folder` in project.yml**: Fonts directory is included as a folder reference resource in the xcodegen project, keeping font files together in the Resources/Fonts/ bundle path.

## How it fits together

`BrandColors.swift` defines `LoopflowPalette` (light/dark/deep-dark) and the environment key. `ConcertoApp.swift` resolves the palette from `AppearanceMode` and injects it at the scene root via `.environment(\.palette, resolvedPalette)`. Every view reads `@Environment(\.palette) private var palette` and uses its tokens. `DesignSystem.swift` provides Typography (using bundled fonts) and button styles that also read from palette.

## Risks and bottlenecks

- **Font registration is fire-and-forget**: If a font file is missing from the bundle, the `guard let url` silently skips it and falls back to system fonts. This is intentional (graceful degradation) but means missing fonts won't be obvious.
- **Deep-dark theme** is defined but not exposed in the UI (no menu option). It exists for future use.

## What's not included

- Roadmap phase 7 (final polish pass) remains open
- No automated visual regression testing
- No font preloading verification (beyond "does the app render correctly")
