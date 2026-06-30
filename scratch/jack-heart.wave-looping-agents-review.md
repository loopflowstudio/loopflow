# Goal primitive gate review

## What was implemented

Added a `goal/` prompt primitive and a `goal` field on waves. Goals resolve from
repo-local files, user files, then builtins, and a wave with `goal: <name>` runs
the rendered goal prompt as its loop body.

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
- Missing goals now surface as `goal not found` instead of falling through as
  `step not found`.

## How it fits together

`Wave.goal` is stored with the wave row and exposed through HTTP DTOs. When lfd
executes a wave run, `build_wave_run_command` chooses the goal path when present:
load goal, render it with flow names and `wave/<name>` as the roadmap handle,
then launch `lf` with an inline prompt. Waves without a goal keep the existing
primary-flow behavior.

## Risks and bottlenecks

- Goal prompts are passed as a single inline CLI argument. That is fine for the
  current small builtins, but very large repo-authored goals could eventually hit
  argv-size limits.
- The primitive coexists with existing `direction` plumbing in this branch. The
  roadmap says goal supersedes direction, but the full removal is outside this
  first cut.
- Concerto UI validation is locally blocked by the UI runner environment. The
  Swift package tests pass; the Xcode UI job failed before completing bootstrap.

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
- `docker version && cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` failed locally: unit tests ran, but the UI runner exited before bootstrapping; logs also showed ATS failures from local Concerto config pointing at `http://100.96.227.95:2486`.
