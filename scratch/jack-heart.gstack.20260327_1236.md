# Stage 1: Import and convert gstack

## Validation

```bash
# Verify 29 gstack steps appear in listing
target/debug/lf --list | sed -n '/gstack/,+31p'

# Verify imported prompt uses loopflow step names
target/debug/lf-prompt --repo . --step gstack:office-hours --surface headless --lfdocs false --diff false --diff-files false | rg 'gstack:(ceo-review|eng-review|design-review)'

# Converter tests
uv run pytest python/tests/test_workstyle_convert.py -v

# Full test suites
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
```

## Done-when check

- `lf gstack:office-hours` resolves via the workstyle source and the loaded prompt references loopflow step names
- `lf --list` shows the 29 gstack steps under the `gstack` source
- Python and Rust test suites pass
