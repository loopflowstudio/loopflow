## Try it!

- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- Inspect the new tend flow surface with `lfq show <wave> --json`, `lf flow tend`, and the built-in tend docs under `rust/loopflow/src/engine/builtins/steps/tend/`

What you should see:
- tend parses as `scan-waves -> or(router: tend/assess)` with `chord`, `reorg`, and `silence` paths
- `ship-roadmap` keeps working with ops inside an `or` sub-flow
- Python and docs no longer expose standalone chord CRUD
