## Try it!

```bash
cargo run --bin lf -- ops pm --help
cargo run --bin lf -- ops pm pull --help
cargo run --bin lf -- ops pm export --help
cargo test -p loopflow ops::pm::tests::pm_export_creates_updates_and_skips_without_recreating_missing_remote_items -- --exact
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
```

What to look for:
- `lf ops pm --help` now advertises the full PM surface, including `pull` and `export`.
- `lf ops pm pull --help` and `lf ops pm export --help` show the wave/`--all` entrypoints used by the new built-in PM steps.
- The focused PM export test proves the local-wins path creates new remote items, updates changed ones, and skips missing remote rows instead of recreating them.
- The broad Rust/Python/E2E/Swift package suites pass locally.

Validation summary:
- `cargo fmt --check` ✅
- `cargo clippy --all-targets --all-features -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (`115 passed`)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (`16 passed`)
- `swift test --package-path swift` ✅
- `xcodebuild test ...` built successfully but did not terminate during this local gate run.
