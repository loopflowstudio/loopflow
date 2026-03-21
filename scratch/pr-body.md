## Try it!

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
.venv/bin/pytest python/tests/
cargo test -p loopflow ops::ingest::tests
swift test --package-path swift
uv run python scripts/concerto-dev.py run-debug
```

Then in Concerto:

1. Open a wave with roadmap items.
2. In the **Roadmap** pane, confirm each card shows a short inline summary.
3. Change a card's priority and verify the item reorders and the underlying file is renamed to the new `1-`/`2-`/`3-`/`4-` prefix.
4. Click the play button on a planned item and verify that exact roadmap file is ingested into `scratch/` and the wave starts running.

Validation run on this branch:

- `cargo fmt --check` ✅
- `cargo clippy --all-targets --all-features -- -D warnings` ✅
- `.venv/bin/pytest python/tests/` ✅ (115 passed)
- `cargo test -p loopflow ops::ingest::tests` ✅ (12 passed)
- `swift test --package-path swift` ✅ (311 tests passed)
- `swift test --package-path swift --filter WaveContentParser` ✅ (6 passed)
- `swift test --package-path swift --filter Multiplexer` ✅ (22 passed)

I could not complete two validation steps inside this sandboxed environment:

- `cargo test --all` hits permission-denied listener / Unix-socket binds in integration tests that open local servers.
- `xcodebuild test` cannot resolve packages here because the sandbox blocks cache writes outside the workspace and has no network access for package fetches.

