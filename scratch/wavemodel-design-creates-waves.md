# Design Creates Waves + Split-Wave

## Objective

Make `lf design` the direct wave creation path and add a dedicated `split-wave` step to decompose oversized waves without changing Rust wave schema models.

## Current state

Implemented:

- `design` wave-plan path now creates `wave/<name>/` directly:
  - `README.md` with Vision / Goals / Risks / Metrics
  - `<name>.yaml` with wave config
  - numbered roadmap files (`01-*.md`, `02-*.md`, ...)
  - `scratch/<branch>.md` for stage 1 implementation
- `design` implement path still exits with `lf implement` guidance
- New interactive `split-wave` step added at `steps/interactive/split-wave.md`
- `split-wave` is registered in built-in discovery metadata and descriptions
- README interactive step table now documents `split-wave`

## Decisions and boundaries

- Keep parent/child linkage at content level (roadmap references), not in Rust schema.
- Keep `split-wave` interactive and confirmation-based before any file writes.
- Keep existing `wave-plan` and `add-to-wave` behavior unchanged for other workflows.
- Keep wave schema discovery unchanged (`wave/<dir>/<dir>.yaml` layout remains compatible).

## Remaining risks

- Prompt-driven file creation quality depends on agent adherence to the wave conventions.
- `discover_target` still falls back from step lookup to flow lookup on any step-load error, which can mask malformed-step diagnostics.
- Local Concerto UI test instability remains in this environment (`ScreenshotPipelineTests.testCapture`, window-not-found).

## Out of scope

- Rust data model/schema changes for parent-child waves
- Automated split-wave decomposition without human confirmation
- Changes to `add-to-wave` or `wave-plan`
