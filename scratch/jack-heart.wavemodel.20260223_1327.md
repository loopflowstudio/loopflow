# Wave Content Model — jack-heart.wavemodel.20260223_1327

Standardize wave README content around `Vision`, `Goals`, `Risks`, `Metrics`, and `Roadmap`, and make core steps use those sections consistently.

## Scope shipped in this branch

- Migrated existing wave READMEs (`agentapi`, `harness`, `loop`, `remote`, `security`) to the standard section model.
- Updated built-in steps to reference the standard sections by name:
  - `ops/update-wave.md`
  - `plan/ingest.md`
  - `plan/kickoff.md`
  - `code/gate.md`
  - `interactive/review.md`
  - `interactive/design.md`
- Added `wave/wavemodel/README.md` and follow-on phase docs:
  - `wave/wavemodel/02-design-creates-waves.md`
  - `wave/wavemodel/03-concerto-nux.md`

## Key design decisions

- Keep configuration/content separation explicit:
  - YAML stays configuration (`flow`, `area`, `direction`, `stimulus`)
  - `wave/<name>/README.md` is strategic content
- Restructure existing README content into the new model instead of rewriting intent.
- Keep step changes additive and backward-compatible (no hard schema/runtime enforcement yet).

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow golden_prompt`
- `grep -l "## Vision\|## Goals\|## Risks\|## Metrics\|## Roadmap" wave/*/README.md`

## Remaining work

- Add `lf design -> wave/<name>/` direct creation path (tracked in `wave/wavemodel/02-design-creates-waves.md`).
- Implement Concerto wave-model UX work (tracked in `wave/wavemodel/03-concerto-nux.md`).
- Decide whether to add automated validation for section presence and quality.

## Risks to watch

- Future README migrations becoming mechanical and flattening nuance.
- Prompt bloat from over-referencing all sections in every step.
- Metrics quality drifting into thin/checklist-only statements.
