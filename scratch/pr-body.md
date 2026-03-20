## Try it!

- `cargo test --test flow_tests`
  - Confirms the shipped built-ins expand to the new routing structure: `garden-or-silent`, the four governance flows, and the iterative `build` loop.
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
  - Confirms redesign bootstrap now configures the redesign wave with `garden-or-silent` across `wave/chord-model/` and `wave/agent-embedding/`.
- `cargo test --all`
- `uv run pytest python/tests/`

Validation on this branch:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (`115 passed`)

## Intent

Ship the governance-oriented flow model instead of the older tend choreography. This branch makes branching and looping first-class in the flow engine, renames the shipped chord-management steps around the garden/wave/VSM vocabulary, and updates the runtime, bootstrap scripts, README, and wave docs so the executable model and the written model stay in sync.

## Assumptions

- Single-path routing is the only execution path needed right now; built-in flows can rely on `xor` while `or` remains a future multi-select construct.
- `wave/mutate` is the shared execution point for garden and VSM governance mutations.
- Structural validation is the right bar for this change: parser/runtime parity, builtin expansion tests, and bootstrap/doc alignment matter more than live-agent proof.

## Key decisions

- Canonicalize mechanical flow items as `op:` while still accepting `ops:` in compatibility parsing.
- Replace `tend/scan-waves` + draft/review/apply-chord with `garden/scan`, `garden/assess`, `wave/mutate`, and `wave/review`.
- Split VSM into four explicit governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) instead of one sequential pipeline.
- Model iterative build flows with an explicit `loop(..., exit: xor(...))` structure in the flow engine and CLI rendering.

## Not included

- Executing multi-select `or` branches
- Worker pools / alternate chord wave modes
- Concrete VSM member-wave configs
- Concurrent ingest / atomic claiming
