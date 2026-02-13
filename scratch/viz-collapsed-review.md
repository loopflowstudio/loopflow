# Viz Phases 1-3 + Polish Tiers 2-4 — Review

Branch: `viz-collapsed` (collapsed into `jack-heart.viz.20260212_1605`)

## What was implemented

Phases 1-3 of the viz roadmap plus Phase 4 polish tiers 2-4. The branch transforms Concerto from system fonts, scattered color references, and uniform button styles into a fully tokenized design system with visual hierarchy.

### Phase 1: Screenshot workflow
- Decoupled screenshot generation from publish (`lf screenshots` step)
- Screenshot coverage: `directions` tags, `--direction`/`--mock-config`/`--tab` flags
- 3 new screenshot states: empty, failed wave, runs tab

### Phase 2: Research & audit
- Docs inventory in `reports/viz/`, stale reports deleted
- Codebase summary (`.lf/summary.md`)
- Visual research and design audit in `reports/viz/`

### Phase 3: Theming system
- `statusNeutral` token added; ~30 system color references replaced with status tokens
- Environment-based palette injection (`@Environment(\.palette)`) — 17 views migrated
- Three palette variants: light, dark, deep wine
- `ThemePreview` for side-by-side comparison in Xcode previews
- Compress pass inlined palette hex values, deleted dead `make(for:)` factory
- Deduplicated `Color.init(hex:)` — single definition in LoopflowCore, made `public`

### Phase 4, Tier 2: Typography tokens
- 6 font files bundled (~2MB): Cormorant Garamond (Regular/Medium/SemiBold), Lato (Regular/Bold), JetBrains Mono (Regular)
- Runtime registration via `CTFontManagerRegisterFontsForURL(.process)` — works for both SPM and xcodegen
- All ~160 system font calls replaced with 5 Typography tokens across 26 files
- Compress pass collapsed `codeSmall()` into `code(size:)`, removed unused `bodyBold()`

### Phase 4, Tier 3: Button hierarchy
- `GhostButtonStyle`: text-only accent color, hover surface fill — for secondary actions (Warp, Cursor, View PR)
- `DestructiveButtonStyle`: outline with statusError color — for destructive actions (Stop, Archive, Cancel)
- `DarkButtonStyle` (existing): filled burgundy — remains for primary actions (Land, Next, Retry)
- 6 button restyle points across WaveDetailPanel, NextActionsBar, InteractiveSessionView

### Phase 4, Tier 4: Token cleanup
- All hardcoded `cornerRadius:` literals replaced with `CornerRadius.sm/md/lg` tokens across 7 files
- `cornerRadius: 6` (LiveOutput, WelcomeWindow) rounded down to `CornerRadius.sm` (4) per VISUAL_DESIGN.md
- Spacing `40` replaced with `Spacing.xxxl + Spacing.sm` in SetupView, WelcomeWindow, CommandPalette
- WaveDetailPanel spacing literals (`20`, `16`, `12`) replaced with `Spacing.xl/lg/md`
- Button styles in DesignSystem.swift now use `CornerRadius.md` instead of literal `8`

## Key choices

**Environment injection for palette.** Enables multi-theme previews — the core requirement for fast visual iteration. One injection point at app root, all views read `@Environment(\.palette)`.

**Three palettes: light, dark, deep wine.** Light and dark are production. Deep wine tests whether burgundy can go moodier.

**Runtime font registration over Info.plist.** `CTFontManagerRegisterFontsForURL` with `.process` scope works for both `swift build` (SPM) and `xcodebuild` (xcodegen). Info.plist only works for Xcode .app bundles.

**Minimal font subset (6 files, not 46).** Only weights used by Typography tokens. Italic Cormorant reserved for future "special moments" per VISUAL_DESIGN.md.

**Five typography tokens.** `heroTitle`, `sectionTitle`, `body`, `caption`, `code` — each with parameterized size. Weight variants via SwiftUI's `.weight()` modifier. `.monospacedDigit()` stays as chained modifier.

**Three button tiers by intent.** Primary (filled), secondary (ghost), destructive (outlined). Maps directly to VISUAL_DESIGN.md button hierarchy. No configuration needed — pick the right style for the action's intent.

**One-pass migrations.** Palette, typography, and buttons each migrated all views in single commits. Incremental migration creates half-migrated codebases.

**Inline hex values.** Compress pass removed intermediate named colors (`loopflowCreamElevated`, etc.) that added indirection without value.

**Token cleanup: round down, not up.** `cornerRadius: 6` becomes `CornerRadius.sm` (4), not a new token. Tighter radii are more refined per VISUAL_DESIGN.md. Small padding values (6, 2 for keyboard shortcut badges) stay as literals — micro-padding below the token grid, not design deviations.

## How it fits together

**Color flow**: `VISUAL_DESIGN.md` -> `StatusColors.swift` -> model files -> views via `@Environment(\.palette)`

**Font flow**: Font files in `Concerto/Fonts/` -> `registerBundledFonts()` at app launch -> `Typography` enum -> all views

**Token hierarchy**: `heroTitle` (serif, 32pt) -> `sectionTitle` (serif, 20pt) -> `body` (sans, 14pt) -> `caption` (sans, 12pt) -> `code` (mono, 13pt)

**Button hierarchy**: `DarkButtonStyle` (primary) -> `GhostButtonStyle` (secondary) -> `DestructiveButtonStyle` (destructive)

**Spatial tokens**: `Spacing` (4pt grid) and `CornerRadius` enums in `DesignSystem.swift` -> all views use semantic names

**Fast iteration**: Change hex in `BrandColors.swift` -> all views update. Use `ThemePreview` for side-by-side comparison.

## Risks

- **Deep wine contrast untested with real data.** Secondary text (`#C8B0A8`) on deep wine surfaces needs verification.
- **Font registration failure is silent.** Falls back to system fonts — degraded but functional.
- **Custom fonts + `.weight()` may synthesize weights.** SwiftUI limitation when exact weight file isn't found.

## What's not included

- Detail view density, empty states, sidebar headers, flow badge density — future polish items
