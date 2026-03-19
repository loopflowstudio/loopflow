---
asana_id: '1213718081081138'
linear_id: 70cde070-1b10-4e97-87b0-e72d35e50d7d
---
# Wave Protocol + Tend/VSM Redesign

## Validation

- `cargo test --test flow_tests` — governance flow structure tests
- `uv run pytest python/tests/test_bootstrap_redesign_script.py -q`
- Full suite: `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all && uv run pytest python/tests/`
