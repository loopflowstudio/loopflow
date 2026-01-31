# Concerto Backlog Restructure

Transform Phase 1 from empty to populated through systematic persona review.

## Problem

Phase 1 (Polish - macOS local) is marked "In progress" but has no backlog items. All existing items in `roadmap/concerto/` are Phase 2 or Phase 3. The Concerto app exists and is functional, but there's no structured way to discover polish work.

The infrastructure for discovery exists:
- Persona directions in `.lf/directions/` (conductor, improviser, returner)
- UX review step in `.lf/steps/ux-review.md`
- Screenshot generation script with manifest

But the loop isn't running: no screenshots generated, no reviews executed, no Phase 1 items produced.

## Approach

**Generate screenshots. Run persona reviews. Populate Phase 1.**

The existing machinery works. Run it:

```bash
# Generate screenshots
python scripts/generate_screenshots.py

# Review each screenshot with each persona
lf ux-review --direction conductor --area docs/screenshots/
lf ux-review --direction improviser --area docs/screenshots/
lf ux-review --direction returner --area docs/screenshots/
```

Output: Phase 1 backlog items in `roadmap/concerto/` with frontmatter:
```yaml
---
status: todo
phase: 1
persona: conductor
screenshot: docs/screenshots/concerto-main.png
---
```

**Fix the README merge conflict.** The README has unresolved conflict markers. Pick a side and commit.

**Document the Phase 1 generation loop.** Add a "Phase 1 items" section to README explaining how Phase 1 work is discovered.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Live app testing | Captures interaction flow, timing | Not repeatable by LLM agents—needs human |
| User interviews | Real user problems | Don't have conductor/improviser/returner users yet |
| Heuristic audit | Deterministic | Loses creative insight from applying persona lens |
| Skip Phase 1 | Ship faster | Phase 1 is the polish phase—skipping defeats the purpose |

## Key decisions

**Screenshots as the artifact.** Static images are reviewable by both humans and LLMs. They create a stable reference for what was reviewed. The alternative (live app review) requires human involvement for every iteration.

**Three screenshots, not exhaustive.** The manifest defines three key views: main dashboard, running wave, waiting wave. This covers the conductor workflow. Improvise screenshots are TODO (requires beta flag). Start with what works.

**LLM review produces candidates, not final items.** The ux-review step outputs backlog item drafts. Human reviews before committing to roadmap/. This catches LLM hallucinations and ensures items are actually actionable.

**Personas are directions, not roles.** Following PROMPT_STYLE.md § Orthogonality, personas apply to any step. `lf implement --direction conductor` is valid—you're implementing with conductor needs in mind. The UX review step is just one use.

> "Managing multiple parallel workstreams. Checking in, not diving deep." — conductor direction

This design applies the conductor lens to UX review.

## Scope

In scope:
- Generate three screenshots (main, running, waiting)
- Run persona reviews to produce Phase 1 candidate items
- Fix README merge conflict
- Document the Phase 1 generation process

Out of scope:
- Improvise mode screenshots (requires beta flag work)
- Web client screenshots (doesn't exist)
- Automated CI for screenshot generation
- User research with real users

## Done when

```bash
# Screenshots exist
ls docs/screenshots/concerto-main.png docs/screenshots/concerto-wave-running.png docs/screenshots/concerto-wave-waiting.png

# Phase 1 items exist
ls roadmap/concerto/*.md | xargs grep -l "phase: 1"

# README has no conflicts
! grep -q "<<<<<<" roadmap/concerto/README.md

# Persona reviews are documented
grep -q "Phase 1 items" roadmap/concerto/README.md
```
