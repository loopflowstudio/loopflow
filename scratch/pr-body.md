## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

For the product flow, run:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Then start two local waves that pause on interactive steps. In Concerto you should see:

- waiting work surface in the attention queue instead of a chat transcript
- one Ghostty-backed tab per interactive wave
- a sidebar with wave metadata, queue pressure, recent run/PR context, and quick actions
- successful terminal exits resume the wave in `lfd`; non-zero exits fail the run

Local note from this gate pass: `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' ...` built successfully and ran unit/package tests, but the `ConcertoUITests` runner still exited early before UI automation bootstrapped on macOS 26.0.1.
