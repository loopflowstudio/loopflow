# Typography Tokens

## Problem

The design system specifies three font families — Cormorant Garamond (headings), Lato (body), JetBrains Mono (code) — but every view uses system fonts. `Typography` tokens exist in `DesignSystem.swift` and are used by exactly zero views. The `Typography` enum is dead code. Meanwhile, 160 font expressions across 22 files use `.font(.caption)`, `.font(.headline)`, `.font(.system(size:))` etc., rendering everything in SF Pro.

This is the single biggest visual gap between the design system and the app. Closing it transforms the entire aesthetic.

## Approach

Two-part implementation, each shippable independently:

**Part 1: Bundle fonts and register at startup.** Add font files to `Concerto/Fonts/`, register them via `CTFontManagerRegisterFontsForURL` in `ConcertoApp.init()`. Update `Package.swift` and `project.yml` to exclude the directory from Swift compilation while including it as bundled resources.

**Part 2: Replace 160 system font calls with Typography tokens.** Mechanical migration across all 22 files. One pass, like the palette migration.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Info.plist `ATSApplicationFontsPath` | Standard macOS pattern for .app bundles | Only works for Xcode builds, not SPM `swift build`. Runtime registration works for both build paths. |
| Keep system fonts, skip bundling | Zero risk, zero effort | Defeats the entire design system. The Typography enum stays dead code forever. |
| Bundle fonts but migrate incrementally | Lower per-PR risk | Creates a half-migrated codebase (same rationale as the palette migration — one pass). |
| Use SF Pro with custom weights/sizes | Native feel, no bundling needed | Looks like every other macOS app. The brand identity is specifically about *not* looking like generic tech. Cormorant Garamond's f-hole italic is a deliberate violin/orchestral reference. |

## Key decisions

**Runtime registration over Info.plist.** `CTFontManagerRegisterFontsForURL` with `.process` scope registers fonts at app launch, visible only to Concerto. Works identically for `swift build` (SPM) and `xcodebuild` (xcodegen). No platform-specific plist keys. ~15 lines in `ConcertoApp.init()`.

**Minimal font weight subset.** Don't bundle all 46 font files from the three families. Bundle only what Typography actually uses:

| Font | Weights needed | Files |
|------|---------------|-------|
| Cormorant Garamond | Regular, Medium, SemiBold (roman only) | 3 |
| Lato | Regular, Bold | 2 |
| JetBrains Mono | Regular | 1 |

6 files total. ~600KB. Italic Cormorant is reserved for "special moments" per VISUAL_DESIGN.md — add it later when there's a use case.

**Extend Typography with weight variants and digit formatting.** The current enum lacks what the codebase actually needs:

| Gap | Current code | New token |
|-----|-------------|-----------|
| Conditional weight | `.font(.system(size: 11, weight: isCurrent ? .semibold : .regular))` | `Typography.body(11).weight(...)` (SwiftUI native) |
| Semibold small icons | `.font(.system(size: 8, weight: .semibold))` | `Typography.caption(8).weight(.semibold)` |
| Monospaced digits | `.font(.caption).monospacedDigit()` | Keep `.monospacedDigit()` as chained modifier — it's a SwiftUI modifier, not a font family concern |

No new token methods needed for weights — SwiftUI's `.weight()` modifier chains onto any `Font`. No new token for `.monospacedDigit()` — it's orthogonal to font family.

**Caption and caption2 collapse to one token.** The codebase uses 69 `.caption` and 27 `.caption2` calls. The difference (12pt vs 10pt) maps to `Typography.caption()` (default 12pt) and `Typography.caption(10)` — the size parameter handles it, no separate token needed.

**Hero sizes vary but the token handles it.** `.system(size: 48)` (setup view), `.system(size: 32)` (quick experiment), `.system(size: 28)` (sidebar stat) all become `Typography.heroTitle(48)`, `Typography.heroTitle(32)`, `Typography.heroTitle(28)`. The default 32 covers the common case.

**DarkButtonStyle gets fixed too.** Line 141 of DesignSystem.swift uses `.font(.subheadline)` — should use `Typography.body()`.

**WaveRow's inline `.custom("Cormorant Garamond", size: 11)` becomes `Typography.caption(11)`.** This is the one place that already references a custom font directly. It should use the token, not a raw string. (Though: an 11pt serif caption is an unusual choice — this may want to be `Typography.body(11)` instead. Using Lato at 11pt is more legible than Cormorant at 11pt. Decision: use `Typography.caption(11)` — Lato is the sans/body family, appropriate for small UI labels.)

## Scope

**In scope:**
- Copy 6 font files into `Concerto/Fonts/`
- Add `registerBundledFonts()` to `ConcertoApp.init()`
- Update `Package.swift` exclude list and `project.yml` sources for the Fonts directory
- Replace all 160 system font calls with Typography tokens
- Fix DarkButtonStyle
- Fix WaveRow's inline custom font reference

**Out of scope:**
- Italic serif (no current use case)
- Font weight tokens (SwiftUI's `.weight()` modifier suffices)
- Typography for LoopflowCore (no views there)
- Ghostty terminal font (separate rendering engine)

## Migration table

Complete mapping from current patterns to tokens:

| Pattern | Count | Token |
|---------|-------|-------|
| `.font(.caption)` | 69 | `Typography.caption()` |
| `.font(.caption2)` | 27 | `Typography.caption(10)` |
| `.font(.headline)` | 12 | `Typography.sectionTitle()` |
| `.font(.subheadline)` | 10 | `Typography.body()` |
| `.font(.title2)` | 4 | `Typography.sectionTitle()` |
| `.font(.system(size: 48))` | 5 | `Typography.heroTitle(48)` |
| `.font(.system(size: 32))` | 1 | `Typography.heroTitle()` |
| `.font(.system(size: 28))` | 1 | `Typography.heroTitle(28)` |
| `.font(.system(size: 16))` | 1 | `Typography.body(16)` |
| `.font(.system(size: 14))` | 1 | `Typography.body()` |
| `.font(.system(size: 12))` | 2 | `Typography.caption()` |
| `.font(.system(size: 12, design: .monospaced))` | 3 | `Typography.code(12)` |
| `.font(.system(size: 11, weight: ...))` | 1 | `Typography.body(11).weight(...)` |
| `.font(.system(size: 10))` | 3 | `Typography.caption(10)` |
| `.font(.system(size: 9, weight: .semibold))` | 3 | `Typography.caption(9).weight(.semibold)` |
| `.font(.system(size: 8, weight: .semibold))` | 1 | `Typography.caption(8).weight(.semibold)` |
| `.font(.system(size: 8))` | 1 | `Typography.caption(8)` |
| `.font(.system(.caption, design: .monospaced))` | 4 | `Typography.codeSmall()` |
| `.font(.system(.caption2, design: .monospaced))` | 1 | `Typography.codeSmall()` |
| `.font(.system(.body, design: .monospaced))` | 1 | `Typography.code()` |
| `.font(.largeTitle)` | 2 | `Typography.heroTitle()` |
| `.font(.title)` | 1 | `Typography.heroTitle()` |
| `.font(.title3)` | 1 | `Typography.sectionTitle()` |
| `.font(.body)` | 1 | `Typography.body()` |
| `.font(.callout)` | 1 | `Typography.body()` |
| `.custom("Cormorant Garamond", size: 11)` | 1 | `Typography.caption(11)` |

Modifiers like `.monospacedDigit()`, `.fontWeight()`, `.bold()` stay as-is — they're orthogonal to the font family.

## Files touched

| File | Changes | Notes |
|------|---------|-------|
| `ConcertoApp.swift` | +15 | Font registration in init |
| `Package.swift` | +1 | Exclude `Fonts/` from sources |
| `project.yml` | +2 | Exclude `Fonts/` from sources |
| `DesignSystem.swift` | +1 | Fix DarkButtonStyle font |
| `WaveDetailPanel.swift` | ~39 | Largest file |
| `WaveRunsTab.swift` | ~15 | |
| `QuickExperimentView.swift` | ~14 | |
| `WaveRow.swift` | ~12 | |
| `WaveSidebar.swift` | ~11 | |
| `SetupView.swift` | ~11 | |
| `InteractiveSessionView.swift` | ~6 | |
| `EmbeddedTerminalPanel.swift` | ~6 | |
| `StepRunner.swift` | ~5 | |
| `CommandPalette.swift` | ~5 | |
| `FlowTypeahead.swift` | ~5 | |
| `FlowProgressPills.swift` | ~5 | |
| `WelcomeWindow.swift` | ~4 | |
| `WaitingStateCard.swift` | ~4 | |
| `TerminalTestWindow.swift` | ~4 | |
| `TypeaheadComponents.swift` | ~3 | |
| `DiagnosticsView.swift` | ~3 | |
| `NextActionsBar.swift` | ~2 | |
| `DirectionTypeahead.swift` | ~2 | |
| `LiveOutput.swift` | ~1 | |
| `AreaTypeahead.swift` | ~1 | |
| `GhosttyTerminalView.swift` | ~3 | Fallback UI only, not terminal |

## Done when

1. `swift build --package-path swift` succeeds
2. `swift test --package-path swift` passes
3. Zero results for `grep -r '\.font(\.system\|\.font(\.caption\|\.font(\.headline\|\.font(\.subheadline\|\.font(\.body\|\.font(\.title\|\.font(\.largeTitle\|\.font(\.callout\|\.font(\.footnote' swift/Concerto/Views/` (all system fonts replaced)
4. Screenshots show custom fonts rendering (not SF Pro fallback)
