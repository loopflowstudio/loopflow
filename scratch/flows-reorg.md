# Flows Reorg (shipped)

Collapsed `flows/{algedonic,build,code,garden,ops,vsm}` + `steps/{code,garden,interactive,ops,plan,vsm,wave}` (13 categories) into `build/`, `govern/`, `ops/` (3 categories). Step names are now bare — `scan`, not `garden/scan`. Full design captured in `wave/flows/README.md`.

## Try it

```bash
# New tree layout
ls rust/loopflow/src/engine/builtins/
# build/{flow,step}  govern/{flow,step}  ops/{flow,step,prompt}

# Flows and steps resolve by bare name
lf validate

# Run any built-in flow or step by its new name
lf gate
lf build
```

## Verify

```bash
# Rust catalog/discovery tests
cargo test -p loopflow

# Python client parity
uv run pytest python/tests/

# E2E smoke
tests/e2e/test_smoke.sh
```

Expected: all green. `flow_tests.rs` now asserts on bare step names.

## Follow-on work

Tracked in `wave/flows/` — placement tuning (item 3), `maybe` primitive to retire `xor(_, silence)` (item 3).
