# Review: docs wave design

## What was implemented

Created the `wave/docs/` wave — a three-sprint plan for fixing loopflow's docs and onboarding. Also updated four prompt files (design.md and update-wave.md, both `.lf/steps/` and builtin copies) to enforce that all sprint files must be created upfront so `ingest` can find them.

### Files changed

**New wave artifacts:**
- `wave/docs/README.md` — vision, strategy, goals, risks, metrics for the docs wave
- `wave/docs/docs.yaml` — wave config (ship-wave flow, docs area, clarity+accessibility directions)
- `wave/docs/01-setup.md` — sprint 1: cross-platform init
- `wave/docs/02-docs.md` — sprint 2: workflow docs and wave guide
- `wave/docs/03-accuracy.md` — sprint 3: reference accuracy pass

**Prompt updates:**
- `.lf/steps/design.md` — added "create every sprint file" note
- `.lf/steps/update-wave.md` — added "no roadmap tables" and "create every sprint file" notes
- `rust/.../interactive/design.md` — builtin copy, same change
- `rust/.../ops/update-wave.md` — builtin copy, same change

**Design doc:**
- `scratch/01-setup.md` — full design for sprint 1 (init rewrite) plus bonus `lf ops ingest` fast-path spec

## Key choices

**Detection-only init.** The core design decision: init runs `command -v` to detect agents, never `brew install` or `npm install`. This makes it safe on any platform.

**Three sprints, front-to-back.** Setup (01) unblocks everything. Docs restructure (02) needs working setup to reference. Accuracy pass (03) cleans up after the restructure.

**Prompt updates are behavioral.** The "create every sprint file" instruction prevents a failure mode where `ingest` can't find work because the design step only created the first sprint file and left the rest as notes in the README.

## How it fits together

The wave is self-contained planning material. Sprint 01's design doc (`scratch/01-setup.md`) is the implementation spec for the first PR off this wave. The prompt updates prevent the failure mode that motivated them — design and update-wave now tell agents to create all sprint files, even sketches, so `ingest` always has files to pick up.

## Risks and bottlenecks

- **Prompt drift.** The `.lf/steps/` copies use "sprint" terminology; the builtin copies use "item"/"stage"/"roadmap". This is pre-existing divergence, not introduced by this branch. The gate pass fixed one instance where the branch additions used "sprint" in the builtin copies (now uses local terminology).
- **scratch/01-setup.md includes two designs.** The init rewrite and the `lf ops ingest` fast-path are in the same doc. The implementing agent should treat the ingest spec as a separate deliverable — it could be split to a separate PR.

## What's not included

- No code changes. This is a design/wave-creation branch.
- Concerto setup fixes (explicitly out of scope per wave vision).
- Gemini CLI support (harness doesn't exist yet).
- `lfd install` changes (already cross-platform).
