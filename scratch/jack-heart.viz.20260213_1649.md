# Sidebar Section Header Weight

## Problem

Sidebar section headers ("Active", "Idle", "Needs Attention", etc.) visually compete with wave names. The headers are organizational scaffolding — they help you parse the list, but they're not the content. The waves are the content.

Looking at the screenshot: "Active" and "Idle" read at nearly the same visual weight as wave names like "swift-falcon". The conductor persona needs to scan waves, not categories. Categories should be invisible infrastructure that you notice only when you need them.

Linear nails this — their section labels are tiny, uppercase, very low opacity. You see "Backlog" and "In Progress" as texture, not text. Your eye slides over them to the issues.

## Approach

Reduce section header prominence through three coordinated changes:

1. **Drop icon opacity from 0.3 to 0.15.** The icons (circle.fill, exclamationmark.triangle.fill, etc.) are the strongest visual element in the headers because they have shape contrast. At 0.15 they become ghost markers — present but not assertive.

2. **Drop text opacity from 0.4 to 0.25.** The uppercase + tracking already signals "this is a label, not content." Lowering opacity reinforces: "you don't need to read me, just sense my presence."

3. **Drop count opacity from 0.3 to 0.2.** The count badge should be the least prominent element — it's useful context but never the thing you're scanning for.

No font size change. The current 9pt Lato caption at uppercase + 0.5pt tracking is already appropriately small. Making it smaller would hurt legibility without improving hierarchy — opacity is the right lever here.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Remove section headers entirely | Maximum density, waves speak for themselves | Loses grouping affordance — when you have 8+ waves, the status groups are genuinely useful for parsing |
| Use divider lines instead of text headers | Even more subtle separation | Too subtle — divider-only makes it hard to understand *why* items are grouped. The label "Needs Attention" carries meaning a line doesn't |
| Reduce font size to 8pt | Smaller = less prominent | Below readable threshold at Retina. Opacity achieves the same subordination without sacrificing legibility |
| Add top margin instead of changing opacity | Creates visual breathing room | Doesn't solve the prominence problem — "Active" still reads as text you should look at |

## Key decisions

**Opacity over size.** The visual-research.md says Linear uses "smaller, lower-opacity" labels, but our 9pt is already smaller than Linear's equivalent. Opacity is our remaining lever. Going below 9pt risks readability.

**Keep uppercase + tracking.** This convention already signals "infrastructure label" vs "content text." It's doing the right work — the opacity just needs to catch up.

**Uniform reduction across icon/text/count.** All three elements drop proportionally. The icon drops the most (0.3 → 0.15, a 50% reduction) because it has the most shape contrast and draws the eye disproportionately.

**No change to "Needs Attention" section.** The exclamationmark.triangle.fill icon for "Needs Attention" gets the same opacity treatment. Its semantic importance comes from the wave rows it contains (which show warning colors), not from the header itself. The header is still there for anyone scanning.

## Scope

- In scope: Opacity values for sidebar section header icon, text, and count in `WaveSidebar.swift:24-40`
- Out of scope: Section header font/size, spacing, row density, flow badge styling, detail pane changes

## Done when

`swift build --package-path swift` compiles. Visual verification via `lf ux-review` shows section headers subordinate to wave names — headers read as texture, not text.
