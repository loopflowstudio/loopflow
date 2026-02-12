# Visual Design Roadmap

Improve Concerto's visual design through the feedback loop defined in VISUAL_DESIGN.md.

## Phases

| Phase | Focus | Status |
|-------|-------|--------|
| 1 | Screenshot workflow | Done |
| 2 | Research & audit | Future |
| 3 | Theming & tooling | Future |
| 4 | Polish | Future |

## Phase 1: Screenshot workflow

Fix the feedback loop. Screenshots are generated during publish but publishing no longer submits PRs, so screenshots drift.

| Item | What it does |
|------|--------------|
| ~~01-decouple-screenshots~~ | Separate screenshot generation from publish |
| ~~02-screenshot-coverage~~ | Add missing states, subdivide by persona |

## Phase 2: Research & audit

Understand what great looks like, find gaps between VISUAL_DESIGN.md and the app.

| Item | What it does |
|------|--------------|
| [03-visual-research](03-visual-research.md) | Research patterns from Notion, Figma, Linear, Arc |
| [04-design-audit](04-design-audit.md) | Audit current design against VISUAL_DESIGN.md |
| ~~05-docs-inventory~~ | Consolidate existing design docs |

## Phase 3: Theming & tooling

Fast iteration on visual changes. Approach TBD based on phase 2 findings.

| Item | What it does |
|------|--------------|
| [06-theming-system](06-theming-system.md) | Theme switching for quick visual iteration |

## Phase 4: Polish

Small, focused diffs (20-200 lines each). One change per PR.

| Item | What it does |
|------|--------------|
| [07-polish](07-polish.md) | Individual polish PRs driven by phases 1-3 |

## Item format

```yaml
---
status: todo | in-progress | done
phase: 1 | 2 | 3 | 4
---
```

## Reference

Design system: `VISUAL_DESIGN.md`
Personas: `.lf/directions/{conductor,improviser,listener}.md`
Screenshot manifest: `scripts/screenshots.yaml`
Screenshot script: `scripts/generate_screenshots.py`
