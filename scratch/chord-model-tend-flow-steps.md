---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# Tend Live Proof + VSM Flow — Validation

## Try it

- `cargo test --test flow_tests builtin_vsm_flow_structure -- --exact` — verifies `vsm` expands to s5 → s4 → s3 → s2 → s1
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q` — verifies bootstrap targets surviving member waves
- Full suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && uv run pytest python/tests/`
