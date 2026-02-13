# Sidebar Header Weight + Detail View Density — Review

## What was implemented

Three related visual polish changes across two files, completing Tiers 5 and 6 of the polish backlog.

### 1. Sidebar section headers (`WaveSidebar.swift`)

Subdued sidebar section headers so wave rows are the visual focus:

- Icons switched from per-section status colors to uniform `.white.opacity(0.3)`
- Removed `color` parameter from `sectionHeader` — icons no longer vary by section
- Titles uppercased with 0.5pt letter-spacing, opacity dropped to 0.4
- Weight dropped from `.medium` to `.regular`
- Hoisted `.font(Typography.caption(9))` to parent `HStack`
- Count text: parentheses removed, opacity lowered to 0.3
- Replaced remaining literal spacing/padding values across sidebar with design tokens

### 2. Detail panel section headers + variable spacing (`WaveDetailPanel.swift`)

Applied the same "infrastructure label" pattern to detail panel sections:

- Extracted `sectionLabel` helper: 10pt, uppercase, tertiary, 0.5pt tracking
- Applied to Commits, Diff, and Live Output headers (replacing 12pt medium secondary)
- Changed outer `VStack(spacing: Spacing.lg)` to `VStack(spacing: 0)` with per-section padding
- Commits→Diff gap tightened to `Spacing.sm` (8pt) — tightly coupled git state
- Diff→OpsActions gap widened to `Spacing.xl` (20pt) — conceptual boundary

### 3. Detail panel cleanup (`WaveDetailPanel.swift`)

Opportunistic deduplication alongside the density changes:

- Config summary wrapped in `palette.surface` card with `CornerRadius.md`
- Extracted `configLabel` helper for area/direction/flow display
- Replaced `HStack { Image; Text }` with `Label` throughout (stop, retry, PR badge, view PR, terminal, IDE, next buttons)
- Extracted `perform(_:_:)` helper — four action methods had identical try/catch/MainActor.run wrappers
- Replaced all remaining literal spacing values with design tokens

## Key choices

- **Uniform white icons over status colors.** Colored section icons competed with wave status indicators in the rows. Section labels are category markers, not status indicators.
- **Uppercase + tracking instead of bold.** Small caps with letter-spacing signals "infrastructure" without weight. Consistent across sidebar and detail panel at 9-10pt.
- **Variable spacing over card grouping.** Commits and Diff cluster via tight spacing (8pt). The gap before ops actions (20pt) signals a conceptual break. Spacing groups without adding visual elements.
- **Config summary card.** Every other running-state section had a card container. The floating config labels were the exception. Three lines of padding/background/clip fix it.
- **`Label` over `HStack { Image; Text }`.** Standard SwiftUI pattern, same visual result, less code. Land button keeps HStack because it conditionally shows a ProgressView spinner.
- **`perform` helper.** Four action methods → one helper + four one-liners. Same error handling, less code.

## How it fits together

Tiers 5 and 6 from `roadmap/viz/07-polish.md`. The sidebar change targets `sectionHeader` and its call sites. The detail panel changes target section headers, inter-section spacing, config summary container, and button/action deduplication. No model or state changes. Two files touched: `WaveSidebar.swift` and `WaveDetailPanel.swift`.

## Risks

- Opacity 0.3-0.4 on burgundy background may be hard to read for users with low-contrast displays. Worth testing with accessibility settings when screenshots are regenerated.
- No automated visual regression test — manual screenshot comparison only.
- Small pill padding values (2pt, 3pt, 6pt) remain as literals in capsule badges. These are intentional sub-grid values for tight badge sizing, not candidates for design tokens.

## What's not included

- No changes to `WaveRow` or wave status indicators (those are already the intended visual focus).
- No changes beyond `WaveSidebar.swift` and `WaveDetailPanel.swift`.
- Ops actions bar layout unchanged — only its spacing relative to other sections changed.
