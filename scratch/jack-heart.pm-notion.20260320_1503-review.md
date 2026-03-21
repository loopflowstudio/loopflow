# Review: Notion PM provider — validation

```bash
cargo test -p loopflow notion
cargo test -p loopflow pm
cargo clippy -p loopflow -- -D warnings
cargo fmt --check
uv run pytest python/tests/
```

All passed locally.
