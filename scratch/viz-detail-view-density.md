# Detail View Density and Hierarchy

## Problem

WaveDetailPanel has flat visual hierarchy. Commits, Diff, Live Output, and Ops Actions all sit at the same level — same header weight, same spacing, same prominence. Nothing guides the eye to what matters for the current state.

Three specific issues:

1. **Section headers are too heavy.** "Commits", "Diff", "Live Output" use `Typography.caption()` + `.fontWeight(.medium)` + `.secondary`. Same treatment the sidebar headers had before Tier 5 fixed them. These are labels, not destinations.

2. **Uniform spacing flattens grouping.** `Spacing.lg` (16pt) between all sections. Commits and Diff are tightly coupled git state — they should cluster. The gap between git state and ops actions should be larger to signal a conceptual boundary.

3. **Config summary floats.** Three `configLabel` items in an HStack with no container. Floats between header and progress card. Every other running-state section has a card container except this one.

## Approach

Two small PRs, each 30-80 lines changed. Together they complete Tier 6.

### PR 1: Subdue section headers + tighten spacing

Apply the Tier 5 sidebar pattern to detail panel section headers. Introduce variable spacing between section groups.

**Section headers** — `commitLogSection`, `diffStatSection`, `liveOutputSection`:

| Property | Before | After |
|----------|--------|-------|
| Font | `Typography.caption()` (12pt) | `Typography.caption(10)` |
| Weight | `.fontWeight(.medium)` | default (`.regular`) |
| Color | `.foregroundStyle(.secondary)` | `.foregroundStyle(.tertiary)` |
| Case | Sentence case | `.textCase(.uppercase)` |
| Tracking | none | `.tracking(0.5)` |

This matches the sidebar `sectionHeader` pattern exactly: smaller, lighter, uppercase with letter-spacing. Labels that mark territory without competing for attention.

**Spacing** — in `blendedView`'s `ScrollView` content:

| Between | Before | After |
|---------|--------|-------|
| Commits → Diff | `Spacing.lg` (16pt) | `Spacing.sm` (8pt) — tightly coupled |
| Diff → Ops Actions | `Spacing.lg` (16pt) | `Spacing.xl` (20pt) — conceptual break |
| All other gaps | `Spacing.lg` (16pt) | `Spacing.lg` (16pt) — unchanged |

Implementation: Change the outer `VStack(spacing: Spacing.lg)` to `VStack(spacing: 0)` and add explicit padding between sections. This gives per-gap control without a wrapper view.

### PR 2: Config summary container

Wrap the running-state config summary in a subtle card to match the progress section card below it.

```swift
// Before
HStack(spacing: Spacing.lg) {
    configLabel("folder", wave.areaDisplay)
    configLabel("target", wave.directionDisplay)
    configLabel("arrow.triangle.branch", wave.flow)
}

// After
HStack(spacing: Spacing.lg) {
    configLabel("folder", wave.areaDisplay)
    configLabel("target", wave.directionDisplay)
    configLabel("arrow.triangle.branch", wave.flow)
}
.padding(Spacing.md)
.background(palette.surface)
.clipShape(RoundedRectangle(cornerRadius: CornerRadius.md))
```

Three lines added. Visually anchors the config summary and creates consistency with the progress section card that follows it.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Group commits + diff into a shared card | Stronger visual grouping | Adds nesting complexity, changes card semantics (sections become sub-sections). Spacing alone achieves the same grouping at lower cost. |
| Remove ops actions bar background | Reduces prominence | Ops actions need some visual boundary — without the card, Land/Next buttons float with no anchor. Better to keep the card but let it breathe with spacing above it. |
| Bold/colored section headers | Creates hierarchy through emphasis | Wrong direction — these headers should recede, not advance. The content (commit messages, diff stats) is what matters. |

## Key decisions

**Match sidebar header pattern exactly.** Tier 5 established uppercase + tracking + lighter opacity as the "infrastructure label" pattern. Extending it to the detail panel creates cross-view consistency. From the wave direction: section labels are category markers, not status indicators.

**Variable spacing over card grouping.** Linear uses spacing alone to group related properties (8px within, 24px between). Cards add visual weight and nesting. Spacing is invisible — it groups without adding elements. This follows the design constraint: "the simplest thing that could work."

**Two PRs, not one.** Headers + spacing is one conceptual change (how sections relate to each other). Config container is independent (how one section looks). Separate PRs keep reviews focused.

## Scope

- In scope: Section header styling, inter-section spacing, config summary container
- Out of scope: Ops actions bar redesign, section reordering, card layouts, model changes

## Done when

- Section headers in detail panel match sidebar header treatment (10pt, uppercase, tracking, tertiary)
- Commits and Diff cluster visually with tighter spacing
- Config summary has a card container matching progress section
- All wave states (idle, running, failed, waiting) render correctly
- `swift build --package-path swift` passes
