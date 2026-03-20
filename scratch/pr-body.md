## Try it!

```bash
cargo test runtime_run_keeps_worktree_clean --lib
cargo test lf_ops_land_writes_cd_directive_for_complete_rotation --test land_tests
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

What you'll see:
- wave-attributed `lf` runs create `.lf/runtime/runs/<run_id>/meta.json` and `events.jsonl`
- `lfd` can replay those journals into `run.*` / `step.*` websocket events
- strict `lf ops land` no longer treats runtime-journal files as dirty worktree state

## Intent

Make manual `lf` runs inside wave worktrees observable by `lfd` without routing execution through the daemon, while preserving normal CLI ergonomics. The journal contract captures run metadata and step lifecycle once, shares it between CLI and daemon code, and now stays invisible to git cleanliness checks so the feature does not break strict ship flows.

## Assumptions

- Wave attribution comes from the sibling worktree naming contract.
- `.lf/runtime/runs/<run_id>/...` is the v1 journal location.
- Polling journal files once per second is acceptable for initial daemon visibility.

## Key decisions

- Added first-class daemon `run.*` / `step.*` events instead of squeezing runtime journals into existing wave/agent events.
- Wrote the journal schema in shared runtime code used by both `lf` and `lfd`.
- Updated each worktree's git exclude to ignore `.lf/runtime/` so observability artifacts do not trip `--strict` flows.

## Not included

- Alternate journal roots or non-worktree attribution paths.
- Daemon-hosted shells / PTYs for real CLI execution.
- A fix for the current macOS Concerto UI-test bootstrap crash (`ConcertoUITests-Runner` exits before connecting on this host).
