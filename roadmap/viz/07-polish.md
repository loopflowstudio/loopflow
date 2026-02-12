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

### Tier 3: Button hierarchy (~60 lines)
- Add `GhostButtonStyle` and `DestructiveButtonStyle` to `DesignSystem.swift`
- Restyle secondary actions (Clone, Next, View PR) as ghost, destructive actions (Stop, Archive) as outline

### Tier 4: Token cleanup (~25 lines)
- Replace hardcoded `cornerRadius:` literals with `CornerRadius` tokens
- Replace hardcoded padding literals with `Spacing` tokens

### Other candidates
- Detail view density and hierarchy
- Empty state refinement
- Sidebar section header weight (smaller, lower-opacity labels)
- Flow badge density (capsule pills vs text-only)

## Process

Each polish item becomes its own PR branch. Use `lf ux-review --direction <persona>` to verify improvements through persona lenses.

## Done when

Each individual polish PR ships independently. No batch requirements — this is an ongoing backlog.
