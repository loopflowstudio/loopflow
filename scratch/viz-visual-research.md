---
status: in-progress
phase: 2
---

# Visual design research

## Problem

Concerto's visual design is made without reference material. The sidebar + status + detail layout works functionally but lacks the visual polish of best-in-class apps. Decisions about color, density, hierarchy, and status visualization are ad hoc.

## Approach

Research four apps that share Concerto's core pattern (sidebar with grouped items + status indicators + detail pane), extract concrete patterns organized by focus area, and write persistent reference material to `reports/viz/`.

**Reference apps**: Linear, Notion, Arc, Figma — chosen because each excels at a different aspect of the sidebar+status+detail pattern.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Hire a designer | External perspective, professional polish | Premature — need reference vocabulary first |
| Copy one app's style wholesale | Fast, coherent | Concerto's domain (autonomous agents) has unique status needs none of these apps solve |
| Screenshot comparison tool | Side-by-side visual diffs | Useful later (Phase 3), but need patterns first |

## Key decisions

**Organized by focus area, not by app.** The audit (04-design-audit) needs to compare Concerto against patterns, not against specific apps. Focus areas: sidebar, status, color, typography, spacing.

**Linear is the primary reference.** Linear's UI redesign is the closest analogue — sidebar with status-grouped items, reduced visual noise, LCH-based color system, high density without clutter. Concerto should aspire to Linear's information density.

**Arc for context switching, not sidebar layout.** Arc's spaces model maps to Concerto's wave groups (Active/Idle/Blocked). The color-coded context isolation is directly applicable.

**Patterns, not pixels.** The output is design principles with concrete examples, not pixel-perfect specifications. The theming system (Phase 3) handles the specifics.

Per the viz wave principles: this research enables the design audit (04-design-audit) which feeds Phase 3 theming and Phase 4 polish. Each finding should map to something actionable.

## Scope

- In scope: sidebar patterns, status visualization, color systems, typography mixing, spacing/density from Linear, Notion, Arc, Figma
- In scope: mapping findings to Concerto's current state and VISUAL_DESIGN.md
- Out of scope: pixel-perfect specs, implementation code, theming system design
- Out of scope: mobile patterns (Concerto is macOS only)

## Done when

`reports/viz/visual-research.md` contains annotated patterns from 4 apps organized by 5 focus areas, with specific observations about how each pattern applies to Concerto.
