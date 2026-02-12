# Typography Tokens — Review

Branch: `jack-heart.viz.20260212_1509`

## What was implemented

Font bundling and typography token adoption across the entire Concerto UI. This closes the single biggest visual gap between the design system (VISUAL_DESIGN.md) and the running app — every view now renders in the specified font families instead of SF Pro.

### Font bundling
- 6 font files (~2MB total) added to `Concerto/Fonts/`: Cormorant Garamond (Regular, Medium, SemiBold), Lato (Regular, Bold), JetBrains Mono (Regular)
- Runtime registration via `CTFontManagerRegisterFontsForURL(.process)` in `ConcertoApp.init()` — works for both SPM and xcodegen builds
- `Package.swift` excludes `Fonts/` from source compilation, includes as `.copy` resource
- `project.yml` excludes `Fonts/` from xcodegen sources

### Typography token migration
- All ~160 system font calls replaced with `Typography` tokens across 26 view files
- Five tokens: `heroTitle()`, `sectionTitle()`, `body()`, `caption()`, `code()` — each accepts an optional size parameter
- Compress pass collapsed `codeSmall()` into `code()` with size parameter, removed unused `bodyBold()`
- `DarkButtonStyle` migrated from `.font(.subheadline)` to `Typography.body()`
- WaveRow's inline `.custom("Cormorant Garamond", size: 11)` migrated to `Typography.caption(11)`

### Verification
- Zero system font calls remain in `swift/Concerto/Views/` (verified via grep)
- `swift build --package-path swift` succeeds
- `swift test --package-path swift` passes (97 tests)
- `cargo test --all` passes, `cargo fmt` and `cargo clippy` clean

## Key choices

**Runtime registration over Info.plist `ATSApplicationFontsPath`.** The plist approach only works for Xcode `.app` bundles, not SPM `swift build`. `CTFontManagerRegisterFontsForURL` with `.process` scope works identically for both build paths. ~15 lines in `ConcertoApp.init()`.

**Minimal weight subset (6 files, not 46).** Only weights actually used by Typography tokens are bundled: Regular/Medium/SemiBold for Cormorant Garamond, Regular/Bold for Lato, Regular for JetBrains Mono. Italic Cormorant reserved for future "special moments" per VISUAL_DESIGN.md.

**Five tokens, not more.** `heroTitle`, `sectionTitle`, `body`, `caption`, `code` cover all usage patterns. Weight variants handled by SwiftUI's `.weight()` modifier. `.monospacedDigit()` stays as a chained modifier — orthogonal to font family.

**`codeSmall` collapsed into `code` during compress.** The separate token existed briefly but added no value — `code(10)` is clearer than `codeSmall()`. Similarly, `bodyBold` was removed in favor of `body().weight(.bold)`.

**One-pass migration.** Same rationale as the palette migration: doing it incrementally creates a half-migrated codebase. All 26 files changed in one commit, compress pass immediately after.

## How it fits together

**Font flow**: Font files in `Concerto/Fonts/` → `registerBundledFonts()` at app launch → fonts available process-wide → `Typography` enum references them by family name → all views use Typography tokens.

**Token hierarchy**: `heroTitle` (serif, 32pt default) → `sectionTitle` (serif, 20pt) → `body` (sans, 14pt) → `caption` (sans, 12pt) → `code` (mono, 13pt). Each token maps to one font family, sizes are parameterized.

**Builds on Phase 3**: The palette migration (`@Environment(\.palette)`) is a prerequisite — views already use environment-injected design tokens. Typography follows the same pattern: centralized definitions, used everywhere.

## Risks and bottlenecks

- **Font registration failure is silent.** If `Bundle.module.url` returns nil for a font file, registration is skipped with `continue`. The app falls back to system fonts for that family — degraded but functional. Could add logging but the failure mode is benign.

- **Custom fonts in SwiftUI `.weight()` may not resolve all weights.** If SwiftUI can't find the exact weight file (e.g., `.semibold` requests CormorantGaramond-SemiBold), it may synthesize the weight. This is a SwiftUI limitation, not a bug — the correct files are bundled.

- **Duplicate `Color.init(hex:)` across modules.** Both `StatusColors.swift` (LoopflowCore) and `BrandColors.swift` (Concerto) define this initializer. Pre-existing, compiles because they're in separate modules. Not addressed on this branch.

## What's not included

- Italic Cormorant Garamond (no current use case)
- Typography for LoopflowCore (no views in that module)
- Ghostty terminal font (separate rendering engine, not SwiftUI)
- Font weight tokens (SwiftUI's `.weight()` modifier suffices)
- Runtime font fallback detection or logging
