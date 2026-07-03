# Gate Review: Wave / Run / Session Reduction

## What Was Implemented

Reduced lfd's runtime model to three product nouns: `Wave`, `Run`, and
`Session`. The branch removes public `AgentLaunch`, collapses `WaveRun` and
`AgentRun` into `Run`, collapses terminal and conversation session concepts
into `Session`, and moves public wire fields/routes to `run_id`, `session_id`,
`/v0/runs`, and `/v0/sessions`.

Python, Rust, Swift, DTO fixtures, docs, website copy, and e2e/regression tests
now use the reduced model. `lfq wave run` and `lfq worker run` are the runtime
surfaces; the old dispatch route and `lf op dispatch` path are gone.

## Key Choices

- No compatibility aliases were kept for removed public nouns or routes. The
  branch follows the repo rule to migrate internal config/API surfaces rather
  than carry old names.
- Conversation transcript/input events remain under `/v0/conversations/*`
  internally, while live attachable control surfaces are exposed as `Session`.
  This keeps "conversation" as a transport detail and "Session" as the product
  noun.
- Queue projection stayed on `RunDto`; stale docs that pointed reviewers at
  `/v0/wave_runs` were updated.
- `session_input_round_trip` uses a deterministic Bash fake Codex app-server and
  serializes env-mutating tests through the existing `EnvGuard`, avoiding a
  parallel-test race without adding production test hooks.
- This gate ran `cargo fmt` and picked up a formatter-only line wrap in
  `rust/loopflow/src/lfd/executor/wave/mod.rs`.

## How It Fits Together

`Wave` is the durable goal/memory/orchestration identity. `Run` is one agent
invocation's execution/result lineage, including queue and PR projection.
`Session` is the attachable live control surface; worker and wave-agent launches
return sessions, and session `use` distinguishes `wave_agent`, `worker`, and
`palette`.

## Risks And Bottlenecks

- The biggest functional gap is intentional and tracked separately:
  `WaveAgentTree.child_waves` is still empty until durable wave ancestry is
  reintroduced in `wave/goals/2-wave-ancestry.md`.
- The route/model rename is broad. Fixture tests across Rust/Python/Swift are
  the main guard against DTO drift.
- The Concerto xcodebuild UI test still needs a rendering-capable macOS runner.
  This headless run did not execute it because the run context explicitly had no
  rendering environment.

## What's Not Included

- Reintroducing durable `Wave.parent_wave_id` and populated child-wave trees.
- A compatibility bridge for `/v0/wave_runs`, `/v0/terminal-sessions`,
  `wave_run_id`, or `terminal_session_id`.
- Replacing vendor cloud runtimes; this branch only aligns loopflow's own API
  and app model around the reduced nouns.

## Validation

Passed:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run python -m pytest python/tests/` (`uv run pytest python/tests/` failed
  to spawn the console script in this local environment; the module entrypoint
  ran 171 tests)
- `swift test --package-path swift` after clearing stale SwiftPM artifact cache
  from the old worktree path
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `cd website && uv run python -m playwright install chromium && uv run python
  -m pytest tests/ -v` (`cd website && uv run python dev.py test` failed at the
  `uv run playwright ...` console-script spawn; the equivalent module path ran
  61 tests, 3 skipped)
- `tests/e2e/test_smoke.sh`
- `uv run python -m pytest tests/e2e/test_api_smoke.py
  tests/e2e/test_concurrent_clients.py -v`
- `uv run python -m pytest tests/regression/ -v`
- `docker version && cargo test -p loopflow docker_`

Not run:

- `cd swift && xcodegen generate && xcodebuild test -project
  LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

  The run was explicitly headless with no rendering environment. Swift package
  tests passed; the UI runner should be left to CI or a local rendered macOS
  session.
