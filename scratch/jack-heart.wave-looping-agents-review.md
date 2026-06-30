# Goal primitive gate review

## What was implemented

Added a `goal/` prompt primitive and a `goal` field on waves. Goals resolve from
repo-local files, user files, then builtins, and a wave with `goal: <name>` runs
the rendered goal prompt as its primary loop body.

The wave DTO mirrors now carry `goal` through Rust, Python, Swift, and the shared
fixture. lfd persists the field, accepts it on create/update/run overrides, and
renders goal runs with available-flow and roadmap context.

The goals wave roadmap was added under `wave/goals/`, and the top-level wave docs
now show `goal: ship-roadmap` in wave config examples.

## Key choices

- Goal resolution mirrors step/flow builtin registration: builtins live under
  category directories, while repo-local `goal/<name>.md` can override the
  builtin `ship-roadmap` goal.
- Goal text stays as prompt prose. Metrics and success criteria live in the
  prompt, not in a new config struct.
- A goal run uses `lf : <rendered prompt>` instead of inventing a separate runner.
  That keeps the first primitive small and reuses the existing launch path.
- Goals replace only the standing primary loop body. Explicit flow overrides from
  manual runs, crons, and triggers still run the requested flow.
- `goal` stays explicit in the wave JSON contract. A wave without a goal
  serializes `"goal": null` instead of omitting the field, matching the Python
  required-nullable mirror.
- Missing goals now surface as `goal not found` instead of falling through as
  `step not found`.

## How it fits together

`Wave.goal` is stored with the wave row and exposed through HTTP DTOs. When lfd
executes a wave run, `build_wave_run_command` chooses the goal path only when the
run snapshot still names the wave's primary flow: load goal, render it with flow
names and `wave/<name>` as the roadmap handle, then launch `lf` with an inline
prompt. Runs without a goal, or runs with an explicit flow override, keep the
existing flow execution path.

## Risks and bottlenecks

- Goal prompts are passed as a single inline CLI argument. That is fine for the
  current small builtins, but very large repo-authored goals could eventually hit
  argv-size limits.
- The primitive coexists with existing `direction` plumbing in this branch. The
  roadmap says goal supersedes direction, but the full removal is outside this
  first cut.
- Concerto UI validation was skipped in this headless gate because no rendering
  environment is available. Swift package tests cover the shared model and
  service contract, but not the full Xcode UI runner.

## What's not included

- No Asana live-roadmap integration.
- No hosted long-running lfd/Ghostty backend.
- No budget/spend cap enforcement.
- No full removal of direction fields or flags.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 152 tests.
- `swift test --package-path swift` passed: 336 Swift Testing tests plus 5 XCTest tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version && cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- Concerto Xcode UI test was not run in this gate because the session has no
  rendering environment.
