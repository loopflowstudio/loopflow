---
source: roadmap/viz/07-polish.md
tier: 4
---

# Token Cleanup

Replace hardcoded `cornerRadius:` and padding literals with design system tokens. ~25 lines across ~10 files.

## Corner Radius

From `reports/viz/design-audit.md` Deviation 3:

| File | Count | Current | Replacement |
|------|-------|---------|-------------|
| `WaveDetailPanel.swift` | 4 | 4, 8, 12 | `CornerRadius.sm`, `.md`, `.lg` |
| `CommandPalette.swift` | 4 | 4, 8, 12 | `CornerRadius.sm`, `.md`, `.lg` |
| `WaveRow.swift` | 2 | 8 | `CornerRadius.md` |
| `DesignSystem.swift` (button styles) | 1 | 8 | `CornerRadius.md` |
| `LiveOutput.swift` | 1 | 6 | `CornerRadius.sm` (round down to grid) |
| `WelcomeWindow.swift` | 1 | 6 | `CornerRadius.sm` (round down to grid) |
| `TypeaheadComponents.swift` | 1 | 4 | `CornerRadius.sm` |

`cornerRadius: 6` appears in LiveOutput and WelcomeWindow. Not a design token. Round down to `CornerRadius.sm` (4) — tighter is more refined per VISUAL_DESIGN.md.

## Spacing

From Deviation 4:

| File | Current | Token |
|------|---------|-------|
| `SetupView.swift` | `40` | `Spacing.xxxl + Spacing.sm` (32+8=40) |
| `WelcomeWindow.swift` | `40` | `Spacing.xxxl + Spacing.sm` |
| `CommandPalette.swift` | `40` | `Spacing.xxxl + Spacing.sm` |
| `WaveDetailPanel.swift` | `20`, `16`, `12` | `Spacing.xl`, `.lg`, `.md` |

Decision: compose `40` from existing tokens rather than adding a new `xxxxl` token. Keeps the token set minimal. Use `Spacing.xxxl + Spacing.sm` in code where a literal `40` appears.

Alternative: just use `Spacing.xxxl` (32) — the 8px difference is negligible and avoids composed values. Prefer this if the views look fine at 32.

## Approach

1. Find all hardcoded `cornerRadius:` literals — replace with `CornerRadius.*`
2. Find all hardcoded padding literals not already using `Spacing.*` — replace
3. Fix `cornerRadius: 6` → `CornerRadius.sm`
4. Verify build compiles

## What not to do

- Don't add new tokens (`xxxxl`, `hero`). Compose from existing ones or use the nearest token.
- Don't refactor views. Only swap literals for tokens.
- Don't change visual appearance — the literal values should map to the same or nearly the same pixel values.
