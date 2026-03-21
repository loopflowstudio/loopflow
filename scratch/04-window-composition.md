# 04: Actionable Roadmap — Validation

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p loopflow ops::ingest::tests
.venv/bin/pytest python/tests/
swift test --package-path swift
swift test --package-path swift --filter WaveContentParser
swift test --package-path swift --filter Multiplexer
uv run python scripts/concerto-dev.py run-debug
```

Then in Concerto:

1. Open a wave with roadmap items.
2. In the **Roadmap** pane, confirm each card shows a short inline summary.
3. Change a card's priority and verify the item reorders and the underlying file is renamed to the new `1-`/`2-`/`3-`/`4-` prefix.
4. Click the play button on a planned item and verify that exact roadmap file is ingested into `scratch/` and the wave starts running.
