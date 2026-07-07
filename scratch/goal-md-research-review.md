# goal-md-research review

## What was implemented

Reworked GOAL.md from a frontmatter-driven "default flow + metrics" record into a wave charter: objective, measures, cron duties, and process judgment live in the prompt body, while frontmatter stays machine config. Retrofitted the live wave charters (`goals`, `architecture`, `concerto`, `memory`, `meta`, `systems`) with substantive objectives and outcome measures instead of re-heading old content.

Removed `primary_flow` from the wave wire/storage model across Rust, Python, Swift, fixtures, and docs. The storage path uses migration `053_drop_wave_primary_flow`; DTO fixtures pin the field removal across languages. User-facing examples in README, docs, website content, and the demo script now show routing judgment in Process instead of `primary_flow` frontmatter.

## Key choices

- Kept `crons: []` as structured frontmatter because the current Rust parser still expects structured cron entries; prose cron strings would make config parsing drop the frontmatter.
- Included `wave/memory/GOAL.md` in the retrofit because it was a live wave still carrying the old shape.
- Kept applied migrations intact and added a forward drop migration, with convergence tolerance for stores that already lack the column.
- Treated charters as the primary deliverable; the code migration is deliberately minimal and mirrors the existing DTO/storage patterns.
- Left `wave/goals/MEMORY.md` out of the PR after gate review. Durable wave memory is server-owned and should be changed through `lf memory`, not by hand-editing the tracked file.

## How it fits together

GOAL.md now carries durable identity and routing judgment for the resident mind. Linear remains the roadmap, MEMORY.md remains server-owned durable context, and dispatched workers inherit the charter instead of a mechanical `primary_flow` default.

The storage and DTO change removes the old default-flow field from `Wave`; run dispatch still records concrete `Run.flow` values where work actually happens. Existing default run construction uses `DEFAULT_WAVE_FLOW` only for synthetic/default run rows, not as wave identity.

## Risks and bottlenecks

- Stores that missed earlier wave migrations rely on the convergence-tolerance path for `053_drop_wave_primary_flow`.
- `wave/goals/MEMORY.md` still mentions the old `goal + primary_flow` model, but MEMORY.md is server-owned and was not edited directly in this gate pass.
- Concerto UI validation is blocked locally: `swift test` passes the 301 Concerto package tests, but the xcodebuild UI target hangs in XCTest/LaunchServices before the single UI test runs (`waiting for workers to materialize`, `Waiting for -runningDidFinish call`). This should be rechecked in CI or a clean macOS UI-test environment.

## What's not included

- No GOAL.md authoring UX or elicitation flow from phase B.
- No deeper cleanup of legacy `flow` config parsing beyond removing `primary_flow`.
- No attempt to rewrite server-owned MEMORY.md by hand.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 54 tests.
- `cargo nextest run --all` passed: 1238 tests.
- `cd website && uv run python dev.py test` passed: 61 passed, 3 skipped.
- `swift test --package-path swift -Xswiftc -gnone` passed: 301 Swift tests.
- `tests/e2e/test_smoke.sh` passed.
- `cd swift && xcodegen generate && xcodebuild test ...` did not complete locally: first run hit a DerivedData linker write error. Retrying with an isolated `-derivedDataPath` built past that, reached `Testing started`, then hung in the UI target with `waiting for workers to materialize` and `Waiting for -runningDidFinish call`; interrupted after about three silent minutes.
