# Review: design creates waves + split-wave

## What was implemented

- Updated interactive `design` step so the wave-plan path now creates `wave/<name>/` directly (README, `<name>.yaml`, roadmap files) instead of writing `scratch/wave-proposal.md`.
- Added a new interactive `split-wave` built-in step to decompose oversized waves into child waves with explicit parent/child coordination guidance.
- Registered `split-wave` in built-in discovery metadata and descriptions so it appears in step listings/help.
- Updated user-facing README interactive step table to include `split-wave`.
- Moved the implementation design artifact into `scratch/wavemodel-design-creates-waves.md` and removed the corresponding wave roadmap item file consumed by this branch.

## Key choices

- **Direct wave materialization in design**: keeps design context and wave creation in one session, removing the `add-to-wave` handoff tax.
- **Interactive split confirmation**: `split-wave` requires user confirmation before writing files to avoid incorrect decomposition boundaries.
- **Content-level parent/child linkage**: split relationships stay in markdown roadmap references; no Rust schema/model changes were introduced.

## How it fits together

`design.md` now defines two terminal paths: implement (scratch doc + commit + `lf implement`) and wave plan (write wave directory + stage 1 scratch doc + commit + run stage/flow). The new `split-wave.md` extends the interactive step catalog, and `lf` step discovery metadata exposes it in CLI listings. README mirrors the new interactive capability for discoverability.

## Risks and bottlenecks

- Prompt instructions now carry more file-creation responsibility; quality depends on agent adherence to README/YAML/roadmap conventions.
- `discover_target` still falls back from step lookup to flow lookup on any step-load error (existing behavior), which can hide malformed step diagnostics behind generic not-found behavior.
- Local `xcodebuild test -scheme Concerto` currently fails in this environment due `ConcertoUITests.ScreenshotPipelineTests.testCapture` (window-not-found), while other suites pass.

## What's not included

- No Rust engine/schema parsing changes for wave hierarchy.
- No changes to `add-to-wave` or `wave-plan` behavior.
- No automated split-wave decomposition; human confirmation remains required.
