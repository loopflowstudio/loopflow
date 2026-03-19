# Review — jack-heart.chord-model.20260318_1226

## What was implemented
- Added five built-in VSM step prompts under `rust/loopflow/src/engine/builtins/steps/vsm/` for s5 through s1.
- Added the built-in `vsm` flow YAML and registered the new namespaced VSM steps so flow expansion can resolve them.
- Documented the new VSM steps and flow in the top-level README and built-in flow docs.
- Updated `scripts/bootstrap-redesign.py` to bootstrap the redesign chord against the current member waves and explicitly configure the redesign wave's `tend` flow and area.
- Extended tests to cover the bootstrap script change and the built-in VSM flow structure.

## Key choices
- Kept VSM as prompt-and-flow composition instead of adding new engine-specific orchestration code. That keeps this branch aligned with loopflow's existing “steps are prompts, flows are YAML” model.
- Registered VSM steps in `NAMESPACED_STEPS` alongside other namespaced built-ins so `load_step` and discovery continue to work without changing the build pipeline.
- Made the redesign bootstrap script idempotent by always calling `update_wave("redesign", ...)` after ensuring the wave exists, so old local wave state is corrected instead of silently reused.
- Added a small shared `assert_step_sequence` helper in Rust flow tests to keep the new flow structure assertion readable.

## How it fits together
The Rust engine change is minimal: it exposes the new VSM step prompt files and `vsm` flow to the existing built-in loading and flow expansion paths. The markdown step files carry the governance logic, the flow YAML orders them, and the README/docs changes make the new built-ins discoverable. The bootstrap script change keeps the redesign chord pointed at the surviving member waves so the existing `tend` path still targets the right area.

## Risks and bottlenecks
- s1 is still prompt-driven orchestration, not a dedicated runtime primitive. Launching subwave runs depends on the agent following the prompt and on existing `lfq` / worktree tooling.
- The branch proves flow registration and prompt availability, but it does not add deeper automated coverage for live VSM execution against a running chord.
- The bootstrap script now rewrites redesign flow/area on every run; that is intentional, but reviewers should confirm the chord should stay on `tend` until VSM becomes the desired primary flow.

## What's not included
- No new engine code for VSM-specific batch launching, scheduler enforcement, or algedonic feedback plumbing.
- No live end-to-end test that launches subwaves or verifies multiple PR creation.
- No change to redesign's primary flow from `tend` to `vsm` yet.

## Validation
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`

## Done-when check
Against `scratch/chord-model-tend-flow-steps.md`, this branch completes the prompt/flow/documentation portion:
- builtin VSM steps exist
- built-in `vsm` flow expands in order
- redesign bootstrap targets the current chord members

The deeper runtime outcomes in that design doc — especially s1 launching tracked subwave runs that each produce their own PR and feed algedonic signals forward — are not implemented as dedicated engine behavior in this branch.
