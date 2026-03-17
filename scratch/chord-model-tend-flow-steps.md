# Tend Flow Steps Validation

## Try it

- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- Inspect `rust/loopflow/src/engine/builtins/flows/tend/tend.yaml`, `rust/loopflow/src/engine/builtins/flows/tend/tend-tune.yaml`, `lfq show <wave> --json`, and the built-in tend docs under `rust/loopflow/src/engine/builtins/steps/tend/`

## What to expect

- `tend` parses as `scan-waves -> or(router: tend/assess)` with `tune` and `silence` paths
- `ship-roadmap` still allows ops inside an `or` sub-flow
- Python and docs no longer expose standalone chord CRUD
- `scan-waves.md` reads lfd state via `lfq show --json` and emits a runtime section
