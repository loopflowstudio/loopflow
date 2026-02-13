# Sidebar Section Header Weight — Review

## What was implemented

Subdued sidebar section headers so wave rows are the visual focus. Five changes to `sectionHeader` in `WaveSidebar.swift`:

1. Icons switched from per-section status colors to uniform `.white.opacity(0.3)`
2. Title weight dropped from `.medium` to default (`.regular`), opacity from 0.6 to 0.4
3. Titles uppercased with 0.5pt letter-spacing — reads as infrastructure, not content
4. Top padding reduced from 12pt to 8pt (`Spacing.sm`), bottom from 4pt to `Spacing.xs`
5. Count text opacity lowered to 0.3, parentheses removed for cleaner appearance

Also removed the `color` parameter from `sectionHeader` since icons no longer vary by section. Hoisted shared `.font(Typography.caption(9))` to parent `HStack`. Replaced remaining literal spacing values in `waveList` with design tokens.

## Key choices

- **Uniform white icons over status colors.** The colored section icons competed with wave status indicators in the rows themselves. Linear's pattern: section labels are category markers, not status indicators.
- **Uppercase + tracking instead of bold.** Small caps with letter-spacing signals "category/infrastructure" without weight. Works at 9pt caption size.
- **0.3/0.4 opacity range.** Low enough to recede, high enough to remain readable against the burgundy sidebar background.

## How it fits together

This is one item in the Tier 5 polish backlog (`roadmap/viz/07-polish.md`). The change only touches `sectionHeader` and its call sites in `waveList`. No model or state changes. Consistent with the design token migration done in earlier tiers.

## Risks

- Opacity 0.3–0.4 on burgundy background may be hard to read for users with low-contrast display settings. The design doc notes this — worth testing with accessibility settings if screenshots are regenerated.
- No automated visual regression test for this change (manual screenshot comparison only).

## What's not included

- Literal padding values in `header` (16/12) and `disconnectedState` (16) were left as-is — outside the scope of section header weight.
- No changes to `WaveRow` or wave status indicators (those are already the intended visual focus).
