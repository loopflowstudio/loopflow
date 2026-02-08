# Visual Design Roadmap

Ordered set of PRs to improve Concerto's visual design. Each item is a self-contained design doc that an implementing session can ship.

## Destination

`roadmap/viz/01-*.md`, `roadmap/viz/02-*.md`, etc. New roadmap section alongside `concerto/` and `rust/`.

## Phases

| Phase | Focus | Items |
|-------|-------|-------|
| 1 | Screenshot workflow | Fix the feedback loop first |
| 2 | Research & audit | Understand what great looks like, find gaps |
| 3 | Theming & tooling | Fast iteration on visual changes |
| 4 | Polish | Declutter, refine, ship |

---

## Phase 1: Screenshot workflow

Screenshot generation process (`generate_screenshots.py`) is fine. The issues are workflow and coverage.

### 01 — Decouple screenshots from publish

**Problem:** Screenshots are generated during `publish.py` but publishing no longer submits PRs. Screenshots get left dirty on main.

**Build:**
- Remove screenshot generation from the release flow (`_release()`)
- Keep `publish.py screenshots` as a convenience alias for the standalone script
- Add a `lf screenshots` step that runs the script and commits results

**Done when:** `lf screenshots` generates fresh screenshots and commits them. Publishing no longer touches screenshots.

### 02 — Revisit screenshot coverage and persona subdivision

**Problem:** Current manifest covers 3 states (main, running, waiting). Missing: improvise mode, empty state, error states. The persona questions point to workflows that aren't captured.

**Build:**
- Audit `screenshots.yaml` against the 3 persona directions — which questions can't be answered from current screenshots?
- Add entries for missing states/workflows
- Uncomment/implement the improvise mode screenshot
- Add `--direction` flag to the screenshot script (default: all). Subdivide screenshots by persona — each persona implies a set of workflows/states to capture
- Organize output by persona (e.g., `docs/screenshots/conductor/`, `docs/screenshots/improviser/`)

**Done when:** `python scripts/generate_screenshots.py --direction conductor` generates the screenshots relevant to that persona. Default generates all.

---

## Phase 2: Research & audit

### 03 — Visual design research

**Problem:** No reference material for what "great" looks like. Visual decisions are made without context.

**Build:**
- Research visual patterns from Notion, Figma, Linear, Arc
- Focus on: sidebar patterns, status visualization, color usage, typography, spacing
- Capture as persistent reference material in `reports/viz/`

### 04 — Audit current design against VISUAL_DESIGN.md

**Problem:** VISUAL_DESIGN.md defines a system. How well does the app follow it?

**Build:**
- Compare current screenshots against design tokens
- Document deviations (known: neon green status indicators / action buttons, should be burgundy or organic accent)
- Identify highest-leverage gaps

### 05 — Existing design docs inventory

Review what's already captured in design mds, reports, and scratch for visual guidance. Consolidate what's useful, discard what's stale.

---

## Phase 3: Theming & tooling

### 06 — Theming system for fast iteration

**Problem:** No way to quickly try different visual approaches and get feedback.

**Build:**
- TBD — needs research from phase 2 to inform approach
- Could be: SwiftUI environment-based theme switching, preview variants, or a simpler "swap these 5 colors" approach
- Key requirement: human can see changes fast, give feedback, iterate

---

## Phase 4: Polish

### 07+ — Individual polish PRs

Small, focused diffs (20-200 lines each). Driven by findings from phases 1-3. Known examples:

- **Neon green -> burgundy/organic accent** for status indicators and action buttons ("run button should be burgundy")
- Flow badges (ship, debug, polish) — color and weight
- Detail view density and hierarchy
- Empty state refinement
- Sidebar visual weight and contrast
- Decluttering

---

## Decisions

- **Research artifacts** persist in `reports/viz/` (survives PR merges, unlike `scratch/`)
- **Persona composition**: Screenshot script takes `--direction` flag, subdivides output by persona. `ux-review` then reviews per-persona screenshots through that lens.
- **Phase 4 granularity**: 20-200 lines per diff. One focused change per PR.
