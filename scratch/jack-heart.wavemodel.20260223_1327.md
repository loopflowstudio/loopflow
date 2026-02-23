# Wave Content Model — jack-heart.wavemodel.20260223_1327

Standardize wave README content around `Vision`, `Goals`, `Risks`, `Metrics` (four sections, nothing else) and establish the convention that the roadmap is the `##-*.md` files alongside the README.

## Scope shipped in this branch

- Migrated existing wave READMEs (`agentapi`, `harness`, `loop`, `remote`, `security`) to the standard four-section model.
- Moved supplementary content (Design Decisions, Architecture, API, Reference Frameworks) from READMEs into relevant roadmap items.
- Created roadmap items for `harness` (previously all inline in README).
- Created `wave/remote/05-concerto-remote-connection.md` (was missing).
- Updated built-in steps to reference README sections and roadmap files:
  - `ops/update-wave.md`
  - `plan/ingest.md`
  - `plan/kickoff.md`
  - `code/gate.md`
  - `interactive/review.md`
  - `interactive/design.md`
- Added `wave/wavemodel/README.md` and follow-on roadmap items:
  - `wave/wavemodel/02-design-creates-waves.md`
  - `wave/wavemodel/03-concerto-nux.md`

## Key design decisions

- **README = Vision, Goals, Risks, Metrics.** Nothing else. The README is strategic context.
- **Roadmap = `##-*.md` files.** Numbered files alongside the README ARE the roadmap. No summary table needed.
- **Supplementary content goes into roadmap items.** Design decisions, architecture diagrams, API docs, reference frameworks — all live in the roadmap item where they're most relevant (typically the foundational first item).
- Keep configuration/content separation explicit:
  - YAML stays configuration (`flow`, `area`, `direction`, `stimulus`)
  - `wave/<name>/README.md` is strategic content
  - `wave/<name>/##-*.md` is the roadmap
- Keep step changes additive and backward-compatible (no hard schema/runtime enforcement yet).

## Verification

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow golden_prompt`
- `grep -l "## Vision\|## Goals\|## Risks\|## Metrics" wave/*/README.md`
- Verify no `## Roadmap` section in any wave README

## Remaining work

- Add `lf design -> wave/<name>/` direct creation path (tracked in `wave/wavemodel/02-design-creates-waves.md`).
- Implement Concerto wave-model UX work (tracked in `wave/wavemodel/03-concerto-nux.md`).
- Decide whether to add automated validation for section presence and quality.

## Risks to watch

- Future README migrations becoming mechanical and flattening nuance.
- Prompt bloat from over-referencing all sections in every step.
- Metrics quality drifting into thin/checklist-only statements.
