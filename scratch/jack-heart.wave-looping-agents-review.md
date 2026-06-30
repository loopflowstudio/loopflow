# Goal primitive gate review

## What was implemented

Added `goal` as the loop body for waves and tightened it into the decided
contract: every wave record now has a required `goal` string, defaulting to
`ship-roadmap`, and lfd always renders that goal when launching a wave run.

Goal prompt files resolve from repo `.lf/goals/`, then user `~/.lf/goals/`, then
builtins. Singular `.lf/goal/` and repo-root `goal/` are intentionally ignored.

IDE handoffs now use `Surface::Ide`, so `lf <step> --ide` sends the target agent
only the task seed, without loopflow's `<lf:voice>` block or run-mode surface
instructions.

## Key choices

- Keep `primary_flow` as the default flow a goal can dispatch; it is no longer a
  fallback loop body.
- Keep update and run-override DTO fields optional, because `None` means "do not
  change this field." The canonical wave record and wire DTO require `goal`.
- Backfill existing database rows to `ship-roadmap` and enforce `NOT NULL`
  instead of carrying nullable compatibility.
- Resolve goals through `.lf/goals/` only. The previous singular and repo-root
  paths were removed rather than kept as shims.
- Treat IDE launches as interactive handoffs owned by the host agent, not as
  loopflow-controlled interactive sessions.

## How it fits together

`Wave.goal` is persisted in the waves table and exposed through Rust, Python,
Swift, and the shared fixture. `build_wave_run_command` loads `wave.goal()`,
renders it with available flows and the `wave/<name>` roadmap handle, and
launches the rendered prompt through the existing inline `lf : ...` path.

Prompt assembly carries a `Surface` enum. `Surface::Ide` is selected before
prompt preparation when `--ide` is present, and both full prompt rendering and
skill-launch seeds omit voice and surface text for that variant.

## Risks and bottlenecks

- Goal prompts still travel as a single inline CLI argument. That is acceptable
  for the current builtin and expected repo goals, but very large goals could
  eventually hit argv-size limits.
- `primary_flow` remains in the model as a dispatch default. Its semantics are
  now narrower, so later UI/docs work should avoid presenting it as the loop
  body.
- Concerto UI validation was skipped in this headless gate because no rendering
  environment is available. Swift package tests cover the model and service
  contract, but not the full Xcode UI runner.

## What's not included

- No `prepare_goal_launch` launcher or full Looping Agent session bootstrap.
- No Asana live-roadmap integration.
- No hosted long-running lfd/Ghostty backend.
- No budget or spend-cap enforcement.
- No deletion of `primary_flow`.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 152 tests.
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
