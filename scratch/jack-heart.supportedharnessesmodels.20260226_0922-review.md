# Review: supported harnesses + model resolution

## What was implemented
- Standardized model terminology to `harness:model` across Rust CLI/executor code paths.
- Extended config/step schemas:
  - `agent_model` is now optional.
  - Added `supported_harnesses` to config.
  - Added `default_model` to step frontmatter and parsed step definitions.
- Updated model resolution to honor step defaults and explicit overrides with fallback to `claude:opus`.
- Added wave-level model controls:
  - `model` and `step_models` in wave config.
  - PATCH `/v0/waves/:id` accepts and persists those fields.
  - Wave executor applies per-step override first, then wave-level override.
- Extended APIs consumed by Concerto:
  - `/v0/flows` now returns step `model`, `default_model`, and repo `supported_harnesses`.
  - Wave DTO includes `model` and `step_models`.
- Updated Swift models/state/UI:
  - Wave model carries `model` + `stepModels`.
  - Flow metadata parses step model/default model info.
  - Step runner includes model picker.
  - Flow progress pills show per-step model override badges.
  - Connection settings shows configured supported harnesses.

## Key choices
- Kept `model:` as hard step requirement and introduced `default_model:` as a soft step suggestion.
- Made `agent_model` optional so user preference can be truly absent (instead of always defaulting in config parsing).
- Implemented wave-level override clearing as:
  - `model: ""` => remove wave-level model key.
  - `step_models: {}` => remove step override map.
- Preserved existing wave YAML keys when updating model fields by editing YAML mapping in place.

## How it fits together
`prepare_launch_prompt` now resolves model using explicit input override, then step-required model, then user config (`agent_model`), then step `default_model`, then system fallback. Wave execution feeds wave-level overrides into that same path so CLI and daemon use one shared resolution mechanism. Concerto reads model metadata from `/v0/flows` + wave DTO, writes overrides through PATCH `/v0/waves/:id`, and reflects them in picker/badges.

## Risks and bottlenecks
- PATCH semantics are still two-state for optional fields (`Some` vs omitted): explicit JSON `null` cannot currently mean "clear".
- Wave model override reads wave YAML per step execution. This is simple and correct, but introduces a small per-step file read.
- Local macOS UI test runner is unstable in this environment (xcodebuild exits 65 due early UITest runner termination before connection), despite unit/integration suites passing.

## What's not included
- No tri-state PATCH contract for `model`/`step_models` (`null` vs omitted).
- No dedicated UI for editing per-step overrides yet (current UI surfaces step override badges and wave-level picker).
- No migration/back-compat shim for old terminology; code and docs were moved to the new naming directly.

## Verification
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` (fails locally: `ConcertoUITests-Runner ... Early unexpected exit`; non-UI Swift suites pass)

## Extra polish added in this gate pass
- Added Rust coverage for clearing persisted wave model fields via empty model/step map.
- Added API smoke coverage for PATCH wave model + step model overrides and clearing behavior.
- Updated `docs/config.md` and init template to document `harness:model` wording and `supported_harnesses`.
