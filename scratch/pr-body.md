## Try it!

```bash
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
```

Create a repo goal override:

```bash
mkdir -p goal
printf 'Drive this wave from the repo goal.\n' > goal/ship-roadmap.md
```

Then create or update a wave with `goal: ship-roadmap`. On the next run, lfd
renders that goal prompt with available flows and the `wave/<name>` roadmap
handle, then launches it through the existing `lf : ...` path.

Validation run during gate:

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `cargo test --all` passed.
- `uv run pytest python/tests/` passed.
- `swift test --package-path swift` passed.
- `tests/e2e/test_smoke.sh` passed.
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` passed.
- `docker version && cargo test -p loopflow docker_` passed.
- Concerto Xcode UI test was attempted and failed locally because the UI runner exited before bootstrapping in this headless/local-config environment.

## Intent

Add `goal` as loopflow's third prompt primitive: a prompt run in a loop. Waves can
now name a goal separately from their primary flow, and lfd uses that goal prompt
as the loop body while preserving existing primary-flow behavior for waves
without a goal.

## Assumptions

Goal prompts are small enough to pass through the existing inline `lf : ...`
launch path for this first cut. Rich roadmap integration is represented by the
`wave/<name>` handle for now; Asana-backed live roadmap reads land later.

## Key decisions

- Resolve repo/user/builtin goals with the same mental model as other loopflow
  prompt artifacts.
- Keep goal content as prose, not structured metric config.
- Persist and mirror `goal` across Rust, Python, Swift, and the shared fixture so
  the HTTP contract remains explicit.
- Surface missing goals as `goal not found`.

## Not included

This does not remove direction plumbing, wire Asana, add hosted looping-agent
sessions, or implement spend/budget controls.
