---
status: todo
phase: 2
---

# Audit current design against VISUAL_DESIGN.md

VISUAL_DESIGN.md defines a complete design system — colors, typography, spacing, status tokens. How well does Concerto follow it?

## Known deviations

- Neon green status indicators / action buttons — should be burgundy or organic accent per VISUAL_DESIGN.md
- Status colors may not match the `success`/`warning`/`error`/`info` tokens

## Build

- Compare current screenshots against design tokens defined in VISUAL_DESIGN.md
- Document deviations: what's used vs what's specified
- Identify highest-leverage gaps (changes that would most improve visual coherence)
- Prioritize by effort/impact for Phase 4 polish PRs

## Output

Audit report in `reports/viz/` with specific deviations and recommended fixes.

## Done when

`reports/viz/` contains an audit report mapping current UI to VISUAL_DESIGN.md tokens, with a prioritized list of deviations to fix.
