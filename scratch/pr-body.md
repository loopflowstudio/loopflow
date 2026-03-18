## Try it!

```bash
cargo test -p loopflow pm::linear
cargo test -p loopflow --test config_tests
cargo test -p loopflow --test land_tests
cargo test -p loopflow --test pr_tests
uv run pytest python/tests/ -q
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
```

What to look for:
- Linear's `PmProvider` implementation exercises project creation, pagination, completion-state lookup, rate-limit retry, and GraphQL error handling.
- Local auth docs now include `lf ops auth configure linear` for PM flows that run through `lf ops`.
- Config/PR/land integration tests now pass even when the developer machine has a personal `~/.lf/config.yaml`.
