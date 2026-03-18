# Branch review: jack-heart.pm.20260317_1653

## Validation

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
cargo test -p loopflow pm::linear
cargo test -p loopflow pm::asana
cargo test -p loopflow pm::tests
cargo test -p loopflow --test config_tests
cargo test -p loopflow --test land_tests
cargo test -p loopflow --test pr_tests
uv run pytest python/tests/ -q
```

All pass. Config/PR/land integration tests now use `EnvGuard::with_home()` to isolate from developer `~/.lf/config.yaml`.
