# Goal-driven waves gate review

## What was implemented

Added goals as the wave loop body. Every wave now carries a required `goal`
string, defaults to `ship-roadmap`, and lfd renders that goal prompt for wave
runs instead of falling back to the run snapshot flow.

Added the first `wave/<name>/goal.md` authored-surface slice: a wave-local
`goal.md` can override `.lf/goals/` and builtins, its frontmatter seeds
`primary_flow`, `mode`, `workers`, `metrics`, PM provider config, and agent
settings, and its body becomes the goal prompt. Legacy repo-seeded crons,
triggers, serialized mode, area, and direction are ignored from that file.

Added `metrics` to the canonical wave record and DTO mirrors so rendered goals
see their success criteria. Migrated the local goals wave from
`wave/goals/goals.yaml` to `wave/goals/goal.md`.

Added `Surface::Ide` so `lf <step> --ide` and skill handoff seeds carry the task
without loopflow's `<lf:voice>` block or surface run-mode instructions.

During gate, fixed a PATCH regression where `step_agents` was written to
`wave/<name>/goal.md` but stripped before building the response DTO.

## Key choices

- Keep `primary_flow` as the default flow a goal can dispatch; it is no longer a
  fallback loop body.
- Keep update and run-override DTO `goal` fields optional, because omission means
  "do not change this field." Canonical wave records and wire DTOs require
  `goal`.
- Backfill existing databases to `ship-roadmap` and enforce `NOT NULL` for
  `waves.goal`.
- Resolve goals from `wave/<name>/goal.md`, repo `.lf/goals/`, user
  `~/.lf/goals/`, then builtins. Singular `.lf/goal/` and repo-root `goal/` are
  intentionally ignored.
- Preserve `agent` and `step_agents` in `goal.md` because the live API still uses
  that file as the source of truth for agent overrides.
- Treat IDE launches as interactive handoffs owned by the host agent, not
  loopflow-controlled interactive sessions.

## How it fits together

`Wave.goal` and `Wave.metrics` persist with the wave row and round-trip through
Rust, Python, Swift, and the shared fixture. `build_wave_run_command` loads
`wave.goal()`, renders it with available flows, `wave/<name>` as the roadmap
handle, and `wave.metrics()`, then launches the prompt through the existing
inline `lf : ...` path.

`read_wave_config` parses `wave/<name>/goal.md` as authored intent. Creation
uses the allowed frontmatter fields to seed the wave record; DTO rendering uses
the same file for agent override display. Old YAML-shaped fields that should no
longer seed runtime behavior are nulled after parse.

## Risks and bottlenecks

- Goal prompts still travel as a single inline CLI argument. Large goals could
  eventually hit argv-size limits.
- `wave/<name>/goal.md` now has two jobs: authored wave intent and storage for
  live agent overrides. That keeps the existing API working but should be
  revisited when runtime state is separated from repo intent.
- `primary_flow` remains in the model as a dispatch default. Later UI/docs work
  should avoid presenting it as the loop body.
- Concerto Xcode UI validation was skipped in this headless gate because no
  rendering environment is available. Swift package tests cover model and
  service contracts, but not the full Xcode UI runner.

## What's not included

- No `prepare_goal_launch` launcher or persistent looping session bootstrap.
- No Asana live-roadmap read/write integration.
- No hosted long-running lfd/Ghostty backend.
- No budget or spend-cap enforcement.
- No deletion of `primary_flow`, wave-level `area`, or wave-level `direction`.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 157 tests.
- `cd website && uv run python dev.py test` passed: 61 tests, 3 skipped.
- `swift test --package-path swift` passed: 5 XCTest tests and 336 Swift Testing
  tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
  passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version` passed.
- `cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- Concerto Xcode UI test was not run because this headless gate has no rendering
  environment.
