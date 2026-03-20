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
