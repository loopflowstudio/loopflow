# Branch Validation — jack-heart.chords.20260317_1049

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`

## What to verify

- `scan-waves.md` reads lfd state via `lfq show --json` and emits a runtime section
- Tend expands as `scan-waves -> or(router: tend/assess)` with `chord`, `reorg`, and `silence`
- `ship-roadmap` still supports ops inside an `or` sub-flow
- Chord CRUD is gone from Python, docs, and lfd HTTP routes

## Still open

- First live `lf tend` run against a registered redesign wave remains tracked in `wave/chord-model/02-tend-flow-steps.md`
