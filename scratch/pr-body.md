## Try it!

```bash
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
uv run pytest tests/regression/ -v
cargo test -p loopflow docker_
```

Create a repo goal override:

```bash
mkdir -p .lf/goals
printf 'Drive this wave from the repo goal.\n' > .lf/goals/ship-roadmap.md
```

Then create or update a wave with `goal: ship-roadmap`. On the next wave run,
lfd renders that goal prompt with available flows and the `wave/<name>` roadmap
handle, then launches it through the existing `lf : ...` path. A wave's
`primary_flow` remains available as the default flow the goal can dispatch; it
is no longer the fallback loop body.

Validation run during gate:

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 152 tests.
- `swift test --package-path swift` passed: 5 XCTest tests and 336 Swift Testing tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version` passed.
- `cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- Concerto Xcode UI test was not run because this headless gate has no rendering environment.

## Intent

Make goals the unconditional loop body for waves. A wave now always carries a
required `goal`, lfd always renders that goal for wave runs, and IDE handoffs no
longer bake loopflow's voice or surface instructions into another agent's task
seed.

## Assumptions

Goal prompts are small enough to pass through the existing inline `lf : ...`
launch path for this unit. Rich roadmap integration is represented by the
`wave/<name>` handle for now; Asana-backed live roadmap reads land later.

The Swift `Wave` model is used as local UI state as well as an API mirror, so
the JSON parsing boundary enforces required `goal` while existing UI initializer
defaults remain for previews and optimistic state.

## Key decisions

- Store `Wave.goal` and `WaveDto.goal` as required strings, defaulting new waves
  to `ship-roadmap`.
- Keep patch/update/run override `goal` fields optional, where omission means
  "leave unchanged."
- Backfill existing database rows to `ship-roadmap` and make the column
  `NOT NULL`.
- Resolve goals only from `.lf/goals/`, `~/.lf/goals/`, and builtins. Singular
  `.lf/goal/` and repo-root `goal/` are not compatibility paths.
- Add `Surface::Ide` so IDE deep links carry the task without `<lf:voice>` or
  surface run-mode instructions.

## Not included

This does not add the goal launcher, wire Asana, add hosted looping-agent
sessions, enforce spend/budget controls, or delete `primary_flow`.
