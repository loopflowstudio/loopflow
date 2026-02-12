# Viz Phases 1-3 + Typography — Review

Branch: `jack-heart.viz.20260212_1509`

## What was implemented

Phases 1-3 of the viz roadmap plus the first Phase 4 polish item (typography tokens). The branch transforms Concerto from system fonts and scattered color references to a fully tokenized design system.

### Phase 1: Screenshot workflow
- Decoupled screenshot generation from publish (`lf screenshots` step)
- Screenshot coverage: `directions` tags, `--direction`/`--mock-config`/`--tab` flags, 3 new states

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

### Phase 4: Typography tokens (Tier 2)
- 6 font files bundled (~2MB): Cormorant Garamond (Regular/Medium/SemiBold), Lato (Regular/Bold), JetBrains Mono (Regular)
- Runtime registration via `CTFontManagerRegisterFontsForURL(.process)` — works for both SPM and xcodegen
- All ~160 system font calls replaced with 5 Typography tokens across 26 files
- Compress pass collapsed `codeSmall()` into `code(size:)`, removed unused `bodyBold()`

## Key choices

**Environment injection for palette.** Enables multi-theme previews — the core requirement for fast visual iteration. One injection point at app root, all views read `@Environment(\.palette)`.

**Three palettes: light, dark, deep wine.** Light and dark are production. Deep wine tests whether burgundy can go moodier.

**Runtime font registration over Info.plist.** `CTFontManagerRegisterFontsForURL` with `.process` scope works for both `swift build` (SPM) and `xcodebuild` (xcodegen). Info.plist only works for Xcode .app bundles.

**Minimal font subset (6 files, not 46).** Only weights used by Typography tokens. Italic Cormorant reserved for future "special moments" per VISUAL_DESIGN.md.

**Five typography tokens.** `heroTitle`, `sectionTitle`, `body`, `caption`, `code` — each with parameterized size. Weight variants via SwiftUI's `.weight()` modifier. `.monospacedDigit()` stays as chained modifier.

**One-pass migrations.** Both palette and typography migrated all views in single commits. Incremental migration creates half-migrated codebases.

**Inline hex values.** Compress pass removed intermediate named colors (`loopflowCreamElevated`, etc.) that added indirection without value.

## How it fits together

**Color flow**: `VISUAL_DESIGN.md` → `StatusColors.swift` → model files → views via `@Environment(\.palette)`

**Font flow**: Font files in `Concerto/Fonts/` → `registerBundledFonts()` at app launch → `Typography` enum → all views

**Token hierarchy**: `heroTitle` (serif, 32pt) → `sectionTitle` (serif, 20pt) → `body` (sans, 14pt) → `caption` (sans, 12pt) → `code` (mono, 13pt)

**Fast iteration**: Change hex in `BrandColors.swift` → all views update. Use `ThemePreview` for side-by-side comparison.

## Risks

- **Deep wine contrast untested with real data.** Secondary text (`#C8B0A8`) on deep wine surfaces needs verification.
- **Font registration failure is silent.** Falls back to system fonts — degraded but functional.
- **Custom fonts + `.weight()` may synthesize weights.** SwiftUI limitation when exact weight file isn't found.
- **Duplicate `Color.init(hex:)` across modules.** Pre-existing; compiles because separate modules.

## What's next (Phase 4 remaining)

- Tier 3: Button hierarchy — `GhostButtonStyle`, `DestructiveButtonStyle` (~60 lines)
- Tier 4: Corner radius / spacing token cleanup (~25 lines)
- Detail view density, empty states, sidebar headers, flow badge density
