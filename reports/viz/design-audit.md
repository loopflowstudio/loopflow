# Design Audit: Concerto vs VISUAL_DESIGN.md

Concerto has a design system (`DesignSystem.swift`, `BrandColors.swift`, `StatusColors.swift`) that matches VISUAL_DESIGN.md's tokens. The tokens are correct — the problem is adoption. Most views bypass the design system and use SwiftUI defaults.

---

## Status: What's Right

The design system infrastructure is solid:

- `StatusColors.swift` defines the four status tokens exactly matching VISUAL_DESIGN.md
- `BrandColors.swift` defines the full light/dark palette with cream/slate backgrounds
- `DesignSystem.swift` defines spacing, corner radius, hit targets, typography, and animation
- `LoopflowPalette` adapts for light/dark mode
- `WaveStatus.running`/`.waiting`/`.failed` correctly use `statusSuccess`/`statusWarning`/`statusError`

The known deviation from the roadmap item ("neon green status indicators") has already been fixed — `WaveStatus.running` uses `statusSuccess` (#2D6A4F), not system green.

---

## Deviation 1: System Colors Instead of Design Tokens

**Impact: High | Effort: Low**

~30 instances of SwiftUI system colors (`.blue`, `.orange`, `.gray`, `.purple`, `.cyan`) where design tokens should be used.

### Critical: Status model layer

| Location | Current | Should be |
|----------|---------|-----------|
| `WaveRunStatus.pending` | `.blue` | `.statusInfo` |
| `WaveRunStatus.completed` | `.gray` | neutral token (missing) |
| `WaveRunStatus.cancelled` | `.orange` | `.statusWarning` |
| `WaveStatus.idle` | `.gray` | neutral token (missing) |
| `WaveStatus.paused` | `.gray` | neutral token (missing) |

### Views using system colors

| File | What | Current | Should be |
|------|------|---------|-----------|
| `WaveSidebar.swift` | "Needs Attention" section icon | `.orange` | `.statusWarning` |
| `WaveSidebar.swift` | "Recent Activity" section icon | `.cyan` | `.statusInfo` |
| `WaveSidebar.swift` | "Active" section icon | `.blue` | `.statusInfo` |
| `LiveOutput.swift` | Arrow-prefixed lines | `.blue` | `.statusInfo` |
| `LiveOutput.swift` | Warning-prefixed lines | `.orange` | `.statusWarning` |
| `InteractiveSessionView.swift` | Interactive badge | `.blue` | `.statusInfo` |
| `WaveDetailPanel.swift` | PR state `.merged` | `.purple` | needs token |
| `WaveDetailPanel.swift` | PR state `.draft` | `.orange` | `.statusWarning` |
| `WaveRunsTab.swift` | PR badge `.merged` | `.purple` | needs token |
| `StepRunner.swift` | Disabled button bg | `Color.gray` | `palette.border` or neutral token |
| `CommandPalette.swift` | Background, secondary text | `.gray` | `palette.textSecondary`, `palette.surfaceMuted` |
| `QuickExperimentView.swift` | Section colors | `.orange`, `.blue` | status tokens |
| `IterationTimeline.swift` | Inactive dots | `.gray` | neutral token |
| `EmbeddedTerminalPanel.swift` | Placeholder fill | `.gray` | `palette.surfaceMuted` |
| `ScreenshotWindow.swift` | Placeholder bg | `Color.gray.opacity(0.2)` | `palette.surfaceMuted` |

### Missing token: neutral/inactive

The design system defines 4 status colors but no neutral/inactive token. SwiftUI's `.gray` appears ~15 times for idle states, disabled controls, and inactive dots. Fix: add `statusNeutral` to `StatusColors.swift` and `VISUAL_DESIGN.md`.

Candidate value: `#8B8B8B` (medium gray that works on both cream and slate backgrounds). Or use `palette.textSecondary` where the gray is serving as secondary text color.

### Missing token: merged/special

`.purple` is used for PR merged state in two places. Not in the design system. Options: (a) use `statusInfo` (cyan — informational), (b) define a `statusMerged` token, (c) use the burgundy accent since merging is a "success" action.

---

## Deviation 2: System Fonts Instead of Typography Tokens

**Impact: High | Effort: Medium**

150+ instances of `.font(.system(size:))`, `.font(.caption)`, `.font(.headline)`, etc. across all views. `Typography` tokens exist and are correct but barely used.

### Worst offenders by count

| File | System font instances | Notes |
|------|----------------------|-------|
| `WaveDetailPanel.swift` | ~30 | Most varied view in the app |
| `WaveRunsTab.swift` | ~18 | Table with many text styles |
| `WaveRow.swift` | ~11 | Sidebar rows |
| `WaveSidebar.swift` | ~10 | Section headers |
| `QuickExperimentView.swift` | ~11 | Step launcher |
| `SetupView.swift` | ~9 | First-run UI |
| `InteractiveSessionView.swift` | ~7 | Terminal panel |
| `StepRunner.swift` | ~5 | Config + run buttons |

### Common patterns to fix

| Current | Replacement |
|---------|-------------|
| `.font(.system(size: 13, design: .monospaced))` | `Typography.code()` |
| `.font(.system(size: 11, design: .monospaced))` | `Typography.codeSmall()` |
| `.font(.caption)` / `.font(.caption2)` | `Typography.caption()` |
| `.font(.headline)` / `.font(.title2)` | `Typography.sectionTitle()` |
| `.font(.subheadline)` / `.font(.body)` | `Typography.body()` |
| `.font(.system(size: 32))` (hero) | `Typography.heroTitle()` |

### Impact

Typography is the most visible deviation. The design system specifies Cormorant Garamond for headings, Lato for body, JetBrains Mono for code. Currently the app renders entirely in SF Pro (Apple system font). Switching to the custom fonts would be the single biggest visual transformation.

### Risk

Custom fonts must be bundled with the app. Verify Cormorant Garamond, Lato, and JetBrains Mono are included as resources in the Xcode project / SPM package. If not bundled, the `Typography` calls silently fall back to system fonts — which is what happens now.

---

## Deviation 3: Hardcoded Corner Radius

**Impact: Low | Effort: Low**

~15 instances of literal `cornerRadius:` values instead of `CornerRadius` tokens. The values are usually correct (4, 8, 12) — they just don't reference the tokens.

| File | Count | Values used |
|------|-------|-------------|
| `WaveDetailPanel.swift` | 4 | 4, 8, 12 |
| `CommandPalette.swift` | 4 | 4, 8, 12 |
| `WaveRow.swift` | 2 | 8 |
| `DesignSystem.swift` (DarkButtonStyle) | 1 | 8 |
| `LiveOutput.swift` | 1 | 6 (not a token value) |
| `WelcomeWindow.swift` | 1 | 6 (not a token value) |
| `TypeaheadComponents.swift` | 1 | 4 |

Special: `cornerRadius: 6` appears twice. Not a design system value. Should be `CornerRadius.sm` (4) or `CornerRadius.md` (8).

---

## Deviation 4: Hardcoded Spacing

**Impact: Low | Effort: Low**

~8 instances of literal padding values instead of `Spacing` tokens. Most are in views that predate the design system.

| File | Value | Token equivalent |
|------|-------|------------------|
| `SetupView.swift` | `40` | Beyond `xxxl` (32) — needs larger token or composition |
| `WelcomeWindow.swift` | `40` | Same |
| `CommandPalette.swift` | `40`, `8` | `Spacing.sm` for 8 |
| `WaveDetailPanel.swift` | `20`, `16`, `12` | `Spacing.xl`, `.lg`, `.md` |

### Missing token: extra-large spacing

`40` appears multiple times for hero/welcome screens. The largest token is `xxxl = 32`. Options: add `xxxxl = 40` or `hero = 48` for full-page layouts.

---

## Deviation 5: Single Button Style

**Impact: Medium | Effort: Medium**

Only one custom button style exists (`DarkButtonStyle` — filled burgundy). All buttons look the same regardless of importance.

### What the research recommends

From `visual-research.md`: Linear uses ghost buttons for secondary actions, filled for primary. Destructive actions use outline/destructive style.

### Current button hierarchy (flat)

| Button | Current style | Appropriate style |
|--------|--------------|-------------------|
| Run | Custom inline (burgundy fill) | Primary (filled) |
| Stop | `DarkButtonStyle` (burgundy fill) | Destructive (outline/red) |
| Clone | `DarkButtonStyle` (burgundy fill) | Secondary (ghost/outline) |
| Land | `DarkButtonStyle` (burgundy fill) | Primary (filled) |
| Next | `DarkButtonStyle` (burgundy fill) | Secondary (ghost) |
| View PR | `DarkButtonStyle` (burgundy fill) | Secondary (ghost) |
| Archive | `DarkButtonStyle` (burgundy fill) | Destructive (outline/red) |

### Needed styles

1. **Primary**: Current `DarkButtonStyle` (burgundy fill, cream text)
2. **Secondary/Ghost**: Text-only or outline, no fill. Uses `palette.accent` for text.
3. **Destructive**: Outline with `statusError` color. For Stop, Archive, Delete.

---

## No Deviations Found

These areas match VISUAL_DESIGN.md correctly:

- **Spacing token definitions** — `Spacing` enum matches spec exactly
- **Corner radius definitions** — `CornerRadius` enum matches spec exactly
- **Hit target definitions** — `HitTarget` enum matches spec
- **Brand colors** — `BrandColors.swift` hex values match spec
- **Status color definitions** — `StatusColors.swift` hex values match spec
- **Dark/light palette** — `LoopflowPalette` matches spec
- **Animation** — `DesignAnimation` respects `reduceMotion`
- **Accessibility modifiers** — `minHitTarget()`, `accessibleButton()`, `keyboardFocusRing()` exist

---

## Prioritized Fix List

Ordered by visual impact per line of code changed.

### Tier 1: High leverage, low effort (ship first)

| # | Fix | Files | Lines | Impact |
|---|-----|-------|-------|--------|
| 1 | Replace system colors in status models | `Wave.swift`, `WaveRun.swift` | ~10 | Every status indicator in the app |
| 2 | Add `statusNeutral` token | `StatusColors.swift`, `VISUAL_DESIGN.md` | ~5 | Eliminates `.gray` from models |
| 3 | Replace system colors in views | 10 views | ~30 | Consistent color language |

### Tier 2: High leverage, medium effort

| # | Fix | Files | Lines | Impact |
|---|-----|-------|-------|--------|
| 4 | Replace system fonts with Typography tokens | All views | ~150 | Biggest visual change if fonts are bundled |
| 5 | Verify custom fonts are bundled | Package.swift, resources | ~5 | Prerequisite for #4 to be visible |
| 6 | Add ghost + destructive button styles | `DesignSystem.swift` | ~40 | Better action hierarchy |

### Tier 3: Low leverage, low effort (clean up)

| # | Fix | Files | Lines | Impact |
|---|-----|-------|-------|--------|
| 7 | Replace hardcoded corner radii with tokens | 7 files | ~15 | Consistency |
| 8 | Replace hardcoded spacing with tokens | 4 files | ~8 | Consistency |
| 9 | Fix `cornerRadius: 6` → 4 or 8 | 2 files | ~2 | Grid alignment |

### Tier 4: Design decisions needed

| # | Fix | Decision |
|---|-----|----------|
| 10 | PR merged color | `statusInfo`, burgundy accent, or new token? |
| 11 | Extra-large spacing token | Add `xxxxl = 40`? Or compose from `xxxl + sm`? |

---

## Recommended PR Sequence

Each PR is small and independent. Ship in order for maximum impact.

1. **`viz/status-color-tokens`** — Tier 1 (#1-3). Add `statusNeutral`, replace all system colors. ~45 lines.
2. **`viz/bundle-fonts`** — Tier 2 (#5). Add Cormorant Garamond, Lato, JetBrains Mono to app resources.
3. **`viz/typography-tokens`** — Tier 2 (#4). Replace system fonts with Typography calls. ~150 lines.
4. **`viz/button-hierarchy`** — Tier 2 (#6). Add GhostButtonStyle + DestructiveButtonStyle. Restyle secondary/destructive buttons. ~60 lines.
5. **`viz/token-cleanup`** — Tier 3 (#7-9). Corner radius + spacing token references. ~25 lines.
