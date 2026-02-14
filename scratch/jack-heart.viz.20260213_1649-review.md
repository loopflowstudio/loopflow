# Review: Sidebar Section Header Weight

## What was implemented

Reduced sidebar section header opacity so headers read as texture, not text. Three values changed in `WaveSidebar.swift:sectionHeader()`:

| Element | Before | After |
|---------|--------|-------|
| Icon | 0.3 | 0.15 |
| Text | 0.4 | 0.25 |
| Count | 0.3 | 0.2 |

## Key choices

**Opacity, not size.** Headers are already 9pt uppercase with 0.5pt tracking — small enough. Going smaller hurts legibility. Opacity subordinates without sacrificing readability.

**Icon drops the most** (50% reduction) because shape contrast draws the eye disproportionately. Text and count drop proportionally less.

**Uniform treatment for all sections**, including "Needs Attention." The warning semantic comes from wave row colors, not the header.

## How it fits together

`sectionHeader()` is the single function that renders all sidebar section headers. The change affects "Needs Attention", "Open PRs", "Recent Activity", "Active", and "Idle" — every grouping in the wave list.

## Risks and bottlenecks

Low risk. The values are CSS-like opacity constants — no logic change, no state change, no API change. If the opacity feels too low on certain displays, adjusting is a one-line fix.

## What's not included

Font size, spacing, row density, flow badge styling, and detail pane changes are all out of scope per the design doc.
