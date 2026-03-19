## Try it!
- `cargo test --test flow_tests builtin_vsm_flow_structure -- --exact`
  - Verifies the new built-in `vsm` flow expands to `vsm/s5 → vsm/s4 → vsm/s3 → vsm/s2 → vsm/s1`.
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
  - Verifies redesign bootstrap now targets the surviving member waves and reconfigures the redesign wave.
- Full validation run on this branch:
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`
  - `cargo test --all`
  - `uv run pytest python/tests/`

## Intent
Add the first built-in viable system model flow to loopflow: five governance prompts (s5 through s1), a `vsm` flow that runs them in order, and documentation/tests so the flow is discoverable and stable. In the same branch, keep the redesign bootstrap script aligned with the current chord structure by removing retired member waves and forcing the redesign wave back onto the intended `tend` flow and area.

## Assumptions
- Loopflow's existing execution model is enough for a first VSM cut: built-in steps remain markdown prompts and flows remain YAML composition.
- Namespaced built-in steps still need explicit registration in `NAMESPACED_STEPS`.
- The redesign chord should currently cover `wave/chord-model/` and `wave/agent-embedding/`, and should remain on `tend` until a later branch chooses to make `vsm` its primary flow.

## Key decisions
- Implemented VSM as prompt composition instead of adding new orchestration code, which keeps the change small and lets the existing engine run `lf vsm` immediately.
- Wrote each VSM level with the same assess/update/route pattern so the flow reads consistently from s5 down to s1.
- Made bootstrap idempotent with an explicit `update_wave("redesign", ...)` so stale local wave state gets corrected instead of preserved.
- Added a focused Rust flow-structure test plus the existing full Rust/Python suites rather than adding brittle live orchestration tests in this branch.

## Not included
- No dedicated runtime implementation for s1 batch launching or PR fan-out.
- No live VSM end-to-end test against lfd.
- No switch of redesign's primary flow from `tend` to `vsm`.
