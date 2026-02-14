# Sidebar Section Header Weight

## Problem

Sidebar section headers ("Active", "Idle", "Needs Attention", etc.) visually compete with wave names. The headers are organizational scaffolding — they help you parse the list, but they're not the content. The waves are the content.

The conductor persona needs to scan waves, not categories. Categories should be invisible infrastructure that you notice only when you need them. Linear nails this — their section labels are tiny, uppercase, very low opacity. You see "Backlog" and "In Progress" as texture, not text.

## What changed

Three opacity values in `WaveSidebar.swift:sectionHeader()`:

| Element | Before | After | Why |
|---------|--------|-------|-----|
| Icon | 0.3 | 0.15 | Shape contrast draws the eye disproportionately — drops the most |
| Text | 0.4 | 0.25 | Uppercase + tracking already signals "label" — opacity reinforces |
| Count | 0.3 | 0.2 | Least important element, should be most subdued |

No font size change. 9pt Lato caption at uppercase + 0.5pt tracking is already appropriately small. Opacity is the right lever — going below 9pt risks readability.

Uniform treatment for all sections including "Needs Attention." The warning semantic comes from wave row colors, not the header.

## Alternatives considered

| Approach | Why not |
|----------|---------|
| Remove headers entirely | Loses grouping affordance for 8+ waves |
| Divider lines instead | Too subtle — "Needs Attention" carries meaning a line doesn't |
| Reduce font to 8pt | Below readable threshold at Retina |
| Add top margin | Doesn't solve prominence — "Active" still reads as text |

## Scope

- In scope: Opacity values for sidebar section header icon, text, and count
- Out of scope: Font/size, spacing, row density, flow badge styling, detail pane
