# Sidebar Section Header Weight

Polish the sidebar section headers to be more subdued — smaller, lower-opacity labels that let the wave rows be the visual focus.

## Context

From `reports/viz/visual-research.md` finding #4:
> Linear uses smaller, lower-opacity section labels to avoid visual competition with the items themselves. The sidebar should emphasize waves, not categories.

Current state (`WaveSidebar.swift:24-42`):
- Icon: `Typography.caption(9)` with colored foreground
- Title: `Typography.caption(10)` + `.fontWeight(.medium)` + `.white.opacity(0.6)`
- Count: `Typography.caption(10)` + `.white.opacity(0.4)`
- Padding: `.leading(8)`, `.top(12)`, `.bottom(4)`

## What to change

1. **Reduce icon prominence** — Icons are colored (status colors), making them compete with wave status indicators. Switch to uniform low-opacity white.
2. **Reduce title weight** — Drop `.fontWeight(.medium)` to `.regular`. Lower opacity from 0.6 to 0.4.
3. **Uppercase labels** — Linear uses uppercase section labels. Small text in uppercase reads as category/infrastructure rather than content.
4. **Tighten vertical spacing** — Current `.top(12)` creates too much separation between groups. Reduce to match `Spacing.sm` (8).
5. **Count text** — Already subtle at 0.4 opacity. Keep as-is or slightly reduce.

## Scope

~20 lines changed in `WaveSidebar.swift`. Only the `sectionHeader` function.

## Quality check

- Section headers should be scannable but not eye-catching
- Wave rows should be the clear visual anchor in the sidebar
- Colored icons should only appear on wave status indicators, not section decorators
- Headers should still be readable for accessibility (test with reduced contrast settings)
