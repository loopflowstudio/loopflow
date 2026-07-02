## Try it!

List and attach to live terminal sessions:

```bash
lfq sessions
lfq sessions <wave-name>
lfq attach <terminal-session-id>
```

Create a wave-local goal:

```bash
mkdir -p wave/demo
cat > wave/demo/goal.md <<'EOF'
---
primary_flow: build
metrics:
  - tests pass
---

Run one loop iteration for the demo wave.
EOF
```

Create or update a wave named `demo`, then run it. lfd renders the goal prompt
with available flows, `wave/demo` as the roadmap handle, and the wave metrics.
`primary_flow` remains the default flow a goal can dispatch; it is not the
fallback loop body.

Validation run during gate:

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed: 164 tests.
- `cd website && uv run python dev.py test` passed: 61 tests, 3 skipped.
- `swift test --package-path swift` passed: 5 XCTest tests and 336 Swift Testing tests.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed: 16 tests.
- `uv run pytest tests/regression/ -v` passed: 4 tests.
- `docker version` passed.
- `cargo test -p loopflow docker_` passed: 11 docker-filtered tests.
- Concerto Xcode UI test was not run because this headless gate has no rendering environment.

## Intent

Make waves goal-driven and make their live sessions reachable from `lfq`. This
branch introduces `goal.md` as the authored loop surface, stores required goal
and metrics fields across DTO mirrors, renders goals through the existing wave
run path, and adds the two CLI verbs needed to inspect and attach to lfd-managed
terminal sessions.

## Assumptions

Goal prompts are small enough to pass through the existing inline `lf : ...`
launch path for this unit. Rich roadmap integration is represented by the
`wave/<name>` handle for now; Asana-backed live roadmap reads land later.

Terminal sessions are tmux-backed when `lfq attach` is useful. The CLI requires
tmux on `PATH` and exact terminal session ids for this unit.

`wave/<name>/goal.md` is the repo-authored wave surface, while crons and triggers
remain live lfd state configured through the API.

## Key decisions

- Store `Wave.goal` and `WaveDto.goal` as required strings, defaulting new waves
  to `ship-roadmap`.
- Add required `metrics` mirrors across Rust, Python, Swift, and fixtures.
- Resolve goals from `wave/<name>/goal.md`, `.lf/goals/`, `~/.lf/goals/`, and
  builtins. Singular `.lf/goal/` and repo-root `goal/` are not compatibility
  paths.
- Use `AttentionItem(kind=interactive)` plus `terminal_session_id` context for
  `lfq sessions` needs-input flags instead of adding a terminal-session status.
- Keep `Client.list_terminal_sessions(statuses=...)` as the Python shape, mapping
  live status filters to the current Rust `active_only=true` query and filtering
  locally.
- Preserve `step_agents` when reading `goal.md` so PATCH responses reflect live
  agent overrides.

## Not included

This does not wire Asana live roadmaps, add the hosted looping-agent supervisor,
enforce spend/budget controls, isolate parallel worker branches, implement short
session-id prefix matching, or delete the still-live wave `area` and `direction`
API/UI fields.
