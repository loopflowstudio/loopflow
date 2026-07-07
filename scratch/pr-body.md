## Try it!

```bash
lf wave goals
lf build "probe the next goal-authored reference build" --wave goals --dispatch
```

Open any `wave/*/GOAL.md`: the charter now leads with Objective, Measures, Cron, and Process. Routing judgment lives in Process; `primary_flow` is gone from the wave DTO/storage shape.

Validation run locally:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
cargo nextest run --all
cd website && uv run python dev.py test
swift test --package-path swift -Xswiftc -gnone
tests/e2e/test_smoke.sh
```

The Concerto UI xcodebuild target built and passed its 301 unit tests, but the UI screenshot target hung locally in XCTest/LaunchServices before the test body ran (`waiting for workers to materialize`). Recheck that target in CI or a clean macOS UI-test environment.

## Intent

Make GOAL.md the wave's durable charter instead of a mixed bag of default flow, metrics, and prompt prose. A wave now names what it exists to do, how it measures progress, what recurring duties it owns, and how it chooses work. The code mirrors that shape by removing `primary_flow` from the wave model and keeping concrete flow names on runs/dispatches where work actually executes.

## Assumptions

Linear remains the live roadmap. MEMORY.md remains server-owned. Cron frontmatter stays structured for the current parser. A second objective belongs in a child wave, not in another GOAL.md section.

## Key decisions

- Added a forward migration for `primary_flow` removal instead of editing historical migrations.
- Kept the code migration minimal; the main product value is the substantive wave-charter rewrite.
- Updated Rust, Python, Swift, and fixture DTOs together so absent `primary_flow` is the only wire shape.
- Removed stale user-facing examples that taught `primary_flow` frontmatter.

## Not included

No GOAL.md elicitation UX, no phase-B onboarding work, no manual edit of server-owned `MEMORY.md`, and no broader cleanup of legacy flow parsing.
