---
status: done
phase: 2
---

# Existing design docs inventory

Visual design guidance is scattered across zero locations (besides `VISUAL_DESIGN.md` itself). Fix it before phase 2 research and audits produce more docs with nowhere to live.

## Problem

The viz wave needs `reports/viz/` for research output (03-visual-research) and audit output (04-design-audit). That directory doesn't exist. Before creating it, verify the existing doc landscape: is anything visual hiding in other reports? Does anything contradict `VISUAL_DESIGN.md`? Are there stale docs that should be cleaned up?

## Inventory

### Visual design (1 file)

| File | Status | Notes |
|------|--------|-------|
| `VISUAL_DESIGN.md` | Current | Complete design system — colors, typography, spacing, animation, accessibility. Single source of truth. |

No visual design content exists anywhere else. No colors, typography, or spacing references in `reports/`. No contradictions.

### UX design (4 files)

| File | Status | Notes |
|------|--------|-------|
| `reports/concerto/03-conduct-ux.md` | Current | Dashboard layout, connect/continue/land flows |
| `reports/concerto/04-improvise-ux.md` | Current | Area picker, step runner, stimulus transition |
| `reports/concerto/07-ux-experiments.md` | Current | Persona test scripts with timing targets |
| `reports/cli/ux-polish.md` | Stale | CLI help text TODOs — no completion tracking, items may be done |

### Architecture (8 files)

| File | Status | Notes |
|------|--------|-------|
| `reports/concerto/01-platform.md` | Current | Platform strategy, Rust lfd evolution |
| `reports/concerto/02-auth.md` | Current | Local vs remote auth model |
| `reports/concerto/05-remote-terminal.md` | Current | gRPC terminal streaming |
| `reports/concerto/06-data-structures.md` | Current | Wave struct, key functions |
| `reports/concerto/08-notifications.md` | Current | Push notification architecture |
| `reports/lfd-reliability.md` | Current | Daemon hardening phases |
| `reports/rust-lfd.md` | Current | Rust vs Python daemon exploration |
| `reports/code-quality.md` | Stale | Cleanup TODOs without dates or completion tracking |

### Strategy (5 files)

| File | Status | Notes |
|------|--------|-------|
| `reports/concerto-vision.md` | Current | Product vision, "softwarist" positioning |
| `reports/concerto/00-overview.md` | Current | Conduct vs Improvise modes |
| `reports/concerto/09-phasing.md` | Current | Four-phase product roadmap |
| `reports/landscape.md` | Current | Competitive landscape (Jan 2026) |
| `reports/target-customer.md` | Current | Softwarist definition |
| `reports/teams-vision.md` | Forward-looking | Symphonia team product — not implemented |
| `reports/terminal-reference.md` | Aging | Claude Code / Codex reference — external tool details may have drifted |

### Index

| File | Status |
|------|--------|
| `reports/concerto/README.md` | Current — accurate index of subdirectory |

## Approach

Three actions:

1. **Create `reports/viz/`** with a README that defines its purpose: visual research and audit artifacts for the viz wave. This is where 03-visual-research and 04-design-audit will write their output.

2. **Delete stale docs.** `reports/cli/ux-polish.md` and `reports/code-quality.md` are TODO lists with no completion tracking. They've served their purpose or been superseded. Delete them — git has the history.

3. **Leave everything else.** The concerto reports are well-organized and current. `VISUAL_DESIGN.md` stands alone as the design system. No consolidation needed because nothing visual was scattered in the first place.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Move UX docs into `reports/viz/` | Groups all visual/UX together | UX workflow docs (conduct, improvise) are about interaction patterns, not visual design. They belong in `reports/concerto/` where they sit alongside architecture and data model docs for the same app. |
| Create `reports/viz/design-system.md` summarizing VISUAL_DESIGN.md | Quick reference | Duplication — VISUAL_DESIGN.md *is* the reference. A summary would drift. |
| Do nothing | No file changes | Phase 2 items need somewhere to write. Creating `reports/viz/` now is prerequisite work. |

## Key decisions

**`reports/viz/` is for research and audits, not the design system.** `VISUAL_DESIGN.md` stays at repo root. It's the governing document. `reports/viz/` holds supporting material — reference research, gap analyses, audit findings. The design system tells you what to do; reports tell you why and what's left.

**Delete stale TODOs rather than updating them.** `ux-polish.md` and `code-quality.md` are unbounded TODO lists. They violate the principle that docs should stay current or be deleted. If items are still relevant, they'll surface in reviews. Git preserves the history.

**No reorganization of existing reports.** The `reports/concerto/` structure is coherent and well-indexed. Moving files to satisfy a viz-centric view would break an organization that works for its actual purpose (Concerto product planning).

## Scope

**In scope:**
- Create `reports/viz/README.md` defining the directory's purpose
- Delete `reports/cli/ux-polish.md`
- Delete `reports/code-quality.md`

**Out of scope:**
- Populating `reports/viz/` with research (that's 03-visual-research)
- Auditing against VISUAL_DESIGN.md (that's 04-design-audit)
- Reorganizing `reports/concerto/`
- Updating `VISUAL_DESIGN.md`

## Done when

```bash
# Directory exists with README
cat reports/viz/README.md

# Stale docs removed
! test -f reports/cli/ux-polish.md
! test -f reports/code-quality.md

# No orphaned visual design docs outside VISUAL_DESIGN.md and reports/viz/
grep -rl "burgundy\|design system\|typography\|spacing.*token" reports/ | grep -v reports/viz/
# (should return nothing)
```
