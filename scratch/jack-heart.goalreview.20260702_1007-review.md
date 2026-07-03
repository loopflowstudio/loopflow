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
- Queue projection stayed on `RunDto`; the gate pass fixed stale docs that still
  pointed reviewers at `/v0/wave_runs`.
- Website product-card media now has a shrink guard so the new homepage product
  image stays inside small mobile viewports.

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
- The Concerto xcodebuild UI test command built successfully and entered test
  execution, but the macOS target runner hung waiting for UI workers in this
  headless/no-rendering environment and was interrupted.

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
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `cd website && uv run python dev.py test`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `uv run pytest tests/regression/ -v`
- `docker version && cargo test -p loopflow docker_`

Attempted:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

  The project generated and built, then xcodebuild hung after "Testing started"
  waiting for the macOS target runner to materialize. Interrupted after several
  quiet minutes; no test assertion failure was produced.
