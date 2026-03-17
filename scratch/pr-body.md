## Try it!

- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test -p loopflow --test config_tests`
- `cargo test -p loopflow --test land_tests`
- `cargo test -p loopflow --test pr_tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- Inspect `rust/loopflow/src/engine/builtins/flows/tend/tend.yaml`, `rust/loopflow/src/engine/builtins/flows/tend/tend-tune.yaml`, `rust/loopflow/src/engine/builtins/flows/code/ship-roadmap.yaml`, and `rust/loopflow/src/engine/builtins/flows/code/ship-roadmap-play.yaml`

What you should see:
- `tend` expands as `scan-waves -> or(router: tend/assess)` with `tune` and `silence`
- `ship-roadmap` expands as `ingest -> or(play, silence)` and still preserves ops inside the selected subflow
- `lf ops land` now reports a missing PR before trying to generate PR copy when `--create-pr` is absent
- config/ops integration tests no longer depend on whatever `~/.lf/config.yaml` happens to contain

## Intent

Rename the chord and roadmap routing branches so the built-in flow language matches the decision being made, not old implementation details. The branch also closes two polish gaps uncovered during gate: remote land should fail fast when there is no PR to land, and the affected Rust integration tests should not inherit the developer machine's global loopflow config.

## Assumptions

- The deleted flow names (`tend-chord`, `ship-roadmap-build`) are internal enough that updating the built-ins, README, and tests is sufficient.
- `silence` is the only no-op route these flows need; if a future pass wants an explicit maintenance route, it should be added back as a deliberate new path.
- Local test environments may have non-default `~/.lf/config.yaml` agent settings, so test harnesses must isolate `HOME` when they mock CLI agents.

## Key decisions

- Chose `tune`/`play` to align with the chord vocabulary already used in redesign docs.
- Removed the implicit `reorg` fallback from both flows instead of renaming it, because the router outcomes here are really “act” vs “stay quiet.”
- Moved the missing-PR check ahead of PR-copy generation in `lf ops land`, so the command does not do unnecessary agent work before surfacing an actionable error.

## Not included

- No behavioral changes to `tend/assess`, `ingest`, or the chord draft/review/apply prompts.
- No live macOS UI-test fix; `xcodebuild test` built the app locally but the UI runner timed out during bootstrap with an authentication-canceled error.
