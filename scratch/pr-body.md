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

## Intent

This change makes the roadmap pane operational instead of informational. The backend can now ingest a specific roadmap item, and the macOS workspace uses that capability to let you reprioritize work, see what each item actually is, and launch work on the exact item you chose without leaving Concerto.

## Assumptions

- Wave roadmap priority is represented by the markdown filename prefix, and renaming that file is the intended source-of-truth update.
- Concerto only edits roadmap priority for local repositories with direct filesystem access.
- Running a roadmap card should use the wave's configured flow when present, falling back to `build` only when the wave has no explicit flow.

## Key decisions

- Kept targeted ingest in Rust so the CLI, daemon, and app all share one ingest implementation.
- Used filename/slug matching for `lf ops ingest --item` so the UI can pass the stable filename while humans can still target items by slug.
- Showed the first few content lines inline instead of requiring expansion for basic roadmap scanning.
- Modeled priority edits as file renames rather than metadata writes.

## Not included

- Drag-to-reorder or any ordering model more granular than the four priority buckets.
- Remote roadmap reprioritization.
- Follow-on multiplexer polish like named layouts, directional focus, or richer diff/markdown panes.
