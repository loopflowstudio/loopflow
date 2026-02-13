# Sidebar Section Header Weight + Detail Panel Polish — Review

## What was implemented

Two focused changes in one branch:

### 1. Sidebar section headers (`WaveSidebar.swift`)

Subdued sidebar section headers so wave rows are the visual focus. Changes to `sectionHeader`:

- Icons switched from per-section status colors to uniform `.white.opacity(0.3)`
- Title weight dropped from `.medium` to default (`.regular`), opacity from 0.6 to 0.4
- Titles uppercased with 0.5pt letter-spacing — reads as infrastructure, not content
- Top padding reduced from 12pt to 8pt (`Spacing.sm`), bottom from 4pt to `Spacing.xs`
- Count text opacity lowered to 0.3, parentheses removed for cleaner appearance

Also removed the `color` parameter from `sectionHeader` since icons no longer vary by section. Hoisted shared `.font(Typography.caption(9))` to parent `HStack`. Replaced remaining literal spacing/padding values across the sidebar (`header`, `disconnectedState`, `waveList`) with design tokens.

### 2. Detail panel cleanup (`WaveDetailPanel.swift`)

- Replaced `HStack { Image; Text }` patterns with `Label` throughout (stop button, PR badge, retry, view PR, terminal/IDE buttons, next button)
- Extracted `configLabel` helper to deduplicate wave config summary (area/direction/flow)
- Extracted `perform(_:_:)` helper to deduplicate four identical async-error-handling wrappers (`landWave`, `nextWave`, `stopWave`, `retryWave`)
- Replaced remaining literal spacing values with design tokens (`Spacing.lg`, `.md`, `.sm`, `.xs`)

## Key choices

- **Uniform white icons over status colors.** The colored section icons competed with wave status indicators in the rows themselves. Linear's pattern: section labels are category markers, not status indicators.
- **Uppercase + tracking instead of bold.** Small caps with letter-spacing signals "category/infrastructure" without weight. Works at 9pt caption size.
- **`Label` over `HStack { Image; Text }`.** Standard SwiftUI pattern, same visual result, less code. Button styles control the final appearance.
- **`perform` helper.** Four action methods had identical try/catch/MainActor.run error handling. One helper, four one-liners.

## How it fits together

Tier 5 polish item from `roadmap/viz/07-polish.md`. The sidebar change targets `sectionHeader` and its call sites. The detail panel change is opportunistic cleanup of the same token-migration and deduplication pattern, scoped to `WaveDetailPanel.swift`. No model or state changes.

## Risks

- Opacity 0.3–0.4 on burgundy background may be hard to read for users with low-contrast display settings. Worth testing with accessibility settings if screenshots are regenerated.
- No automated visual regression test (manual screenshot comparison only).
- Small pill padding values (2pt, 3pt, 6pt) remain as literals in capsule badges — these are intentional sub-grid values for tight badge sizing, not candidates for design tokens.

## What's not included

- No changes to `WaveRow` or wave status indicators (those are already the intended visual focus).
- No changes to other views beyond `WaveSidebar.swift` and `WaveDetailPanel.swift`.
