# Review — jack-heart.wavemodel.20260223_1327

## What was implemented

- Migrated the five existing wave READMEs (`agentapi`, `harness`, `loop`, `remote`, `security`) to the standard content model: `## Vision`, `## Goals`, `## Risks`, `## Metrics`, `## Roadmap`.
- Updated six built-in step prompts to reference these sections by name:
  - `ops/update-wave.md`
  - `plan/ingest.md`
  - `plan/kickoff.md`
  - `code/gate.md`
  - `interactive/review.md`
  - `interactive/design.md`
- Added `wave/wavemodel/README.md` plus follow-on phase docs (`02-design-creates-waves.md`, `03-concerto-nux.md`) to stage subsequent work.

## Key choices

- **Restructure, don’t rewrite:** existing wave content was moved into the new section taxonomy, while tactical sections (Architecture, Design Decisions, Core components, etc.) were preserved.
- **Convention-first rollout:** no YAML/schema/runtime changes were introduced in this branch; prompts became section-aware without enforcing hard validation.
- **Backward-compatible prompt edits:** updates were additive and surgical so waves with partial README structure remain operable.

## How it fits together

Wave YAML remains configuration (`flow/area/direction/stimulus`), while `wave/<name>/README.md` is now the strategic source of truth. Planning and review steps were updated so agents read and maintain the same named sections consistently. This aligns ingest/kickoff/design/update/review/gate behavior around one shared wave content model.

## Risks and bottlenecks

- README migrations can still accidentally flatten nuance if future edits become mechanical.
- Section-aware prompts could become verbose if later changes over-reference all sections in every step.
- Metrics sections are intentionally thin in some waves; they need iterative hardening to stay useful.
- No automated validator enforces section presence yet, so consistency still depends on process discipline.

## What's not included

- No Rust/Python runtime or schema changes.
- No enforcement that wave READMEs must include all five sections.
- No direct `lf design -> wave/<name>/` creation path yet (tracked in `wave/wavemodel/02-design-creates-waves.md`).
- No Concerto UI implementation yet (tracked in `wave/wavemodel/03-concerto-nux.md`).

## Wave alignment

This branch advances `wave/wavemodel` Goals by standardizing README structure and making step prompts section-aware. It directly addresses the wave Risk around prompt overreach by limiting each step to relevant section references. Observable Metrics progress: all five existing wave READMEs now contain Vision/Goals/Risks/Metrics/Roadmap headings.

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow golden_prompt`
- `grep -l "## Vision\|## Goals\|## Risks\|## Metrics\|## Roadmap" wave/*/README.md`
