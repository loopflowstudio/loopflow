---
status: in-progress
phase: 4
---

# Polish

Small, focused diffs (20-200 lines each). One change per PR. Driven by findings from Phases 1-3.

## Candidates from design audit

Prioritized by visual impact per line changed. Full details in `reports/viz/design-audit.md`.

### ~~Tier 1: Status color tokens~~ (done in Phase 3)

### ~~Tier 2: Typography tokens~~ (done — fonts bundled, 160 system font calls replaced)

### ~~Tier 3: Button hierarchy~~ (done — ghost, destructive, primary styles applied across 6 restyle points)

### ~~Tier 4: Token cleanup~~ (done — cornerRadius and padding literals replaced with design tokens)

### ~~Tier 5: Sidebar section header weight~~ (done — subdued headers, uniform icons, uppercase+tracking)

### ~~Tier 6: Detail view density and hierarchy~~ (done — section labels, variable spacing, config card)

### Tier 7: Empty state refinement (in progress)

### Other candidates
- Flow badge density (capsule pills vs text-only)

## Process

Each polish item becomes its own PR branch. Use `lf ux-review --direction <persona>` to verify improvements through persona lenses.

## Done when

Each individual polish PR ships independently. No batch requirements — this is an ongoing backlog.
