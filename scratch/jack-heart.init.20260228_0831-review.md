# Review: docs wave design

## What was implemented

Created `wave/docs/` — a three-sprint wave for fixing loopflow's docs and onboarding. Updated four prompt files to enforce that all sprint files must be created upfront so `ingest` has files to pick up.

### Files changed

**Wave artifacts (5 new files):**
- `wave/docs/README.md` — vision, strategy, goals, risks, metrics
- `wave/docs/docs.yaml` — wave config (ship-wave flow, clarity+accessibility directions)
- `wave/docs/01-setup.md` — sprint 1: cross-platform init
- `wave/docs/02-docs.md` — sprint 2: workflow docs and wave guide
- `wave/docs/03-accuracy.md` — sprint 3: reference accuracy pass

**Prompt updates (4 files):**
- `.lf/steps/design.md` — "create every sprint file" instruction
- `.lf/steps/update-wave.md` — "no roadmap tables" + "create every sprint file"
- `rust/.../interactive/design.md` — builtin copy, same change (uses "roadmap files"/"stage" terminology)
- `rust/.../ops/update-wave.md` — builtin copy, same change (uses "item files" terminology)

**Design doc:**
- `scratch/01-setup.md` — full spec for sprint 1 (init rewrite) + `lf ops ingest` fast-path

## Key choices

**Detection-only init.** Init runs `command -v` to detect agents, never installs anything. Makes it safe on any platform.

**Three sprints, front-to-back.** Setup (01) unblocks everything. Docs restructure (02) needs working setup to reference. Accuracy pass (03) cleans up after the restructure.

**Prompt updates prevent a real failure mode.** Without the "create every sprint file" instruction, design sessions would write the first sprint as a detailed doc and leave the rest as notes in the README. `ingest` then has nothing to pick up for sprints 2 and 3.

**Each prompt copy uses its own terminology.** `.lf/steps/` says "sprint files"; builtin copies say "roadmap files" or "item files". This is pre-existing divergence — the additions match each file's conventions.

## How it fits together

The wave is self-contained planning material. Sprint 01's design doc (`scratch/01-setup.md`) is the implementation spec for the first PR. The prompt updates are the behavioral fix that motivated them — design and update-wave now tell agents to create all sprint files, even sketches.

## Risks and bottlenecks

- **Prompt terminology divergence.** `.lf/steps/` copies use "sprint"; builtin copies use "item"/"stage"/"roadmap". Pre-existing, not introduced here.
- **scratch/01-setup.md has two designs.** The init rewrite and `lf ops ingest` fast-path are in the same doc. The implementing agent should treat ingest as a separate deliverable.
- **Init prompt is LLM-executed.** Cross-platform detection in a prompt is inherently less reliable than compiled code. Mitigated by keeping scope to `command -v` checks only.

## What's not included

- No code changes — this is a design/wave-creation branch
- Concerto setup fixes (out of scope per wave vision)
- Gemini CLI support (harness doesn't exist)
- `lfd install` changes (already cross-platform)
