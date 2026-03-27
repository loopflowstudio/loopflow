# Gate review: gstack stage 1 import

## Validation

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
target/debug/lf --list | sed -n '/gstack/,+31p'
target/debug/lf-prompt --repo . --step gstack:office-hours --surface headless --lfdocs false --diff false --diff-files false | rg 'gstack:(ceo-review|eng-review|design-review)'
```

## Done-when check

- `lf gstack:office-hours` path is resolvable via the workstyle source and the loaded prompt now points at loopflow step names
- `lf --list` shows the 29 gstack steps under the `gstack` source
- Python and Rust test suites pass after the converter cleanup and reference rewrites
