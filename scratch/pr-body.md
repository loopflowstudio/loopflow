## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

For the product flow:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Start two local waves that pause on interactive steps. Verify:

- selecting a wave opens a work surface instead of a terminal takeover
- the native work/session view stays the default tab
- a Ghostty-backed Terminal tab appears only when that wave has an active terminal session
- the terminal sidebar shows wave metadata, queue pressure, recent run/PR context, and quick actions
- terminal exit 0 resumes the wave in `lfd`; non-zero exits fail the run
- no-wave-selected still lands on the repo-wide attention queue with live backend data

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (113 passed)
- `swift test --package-path swift` ✅ (243 passed)
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (smoke pass + 16 passed)
- `xcodebuild test ...` built the app and unit suites, but the UI runner hung before establishing connection in this no-rendering environment
