## Try it!

```bash
cargo build --bin lf
target/debug/lf goal concerto --tmux
tmux attach-session -t lf-loopflow-concerto
target/debug/lf goal concerto --tmux    # idempotent: reprints the same handle
```

Then launch Concerto against this repo and kill lfd. Disk-authored waves should still render, and selecting one should launch/attach through `lf goal <wave> --tmux`.

Validation run during gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run python -m pytest python/tests/
swift test --package-path swift
swift test --package-path swift --filter PortfolioRepoStateTests
uv run python scripts/check_swift_multiplatform_boundaries.py
uv run python -m ruff check scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py
uv run python -m py_compile scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py
uv run python -m pytest tests/regression/ -v
tests/e2e/test_smoke.sh
uv run python -m pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
cd website && uv run python dev.py test
docker version && cargo test -p loopflow docker_ -- --nocapture
```

The Xcode project path was attempted but is not green in this headless run. The UI screenshot test never observed a window, and the hosted unit bundle hit runner/app-host delays plus saved remote-connection and voice model-prep timeouts. The Swift package suite passed, including the touched `PortfolioRepoStateTests`.

## Intent

Make Concerto's basic wave UX work without lfd in the launch/attach path. Waves are now repo-backed (`repos: [RepoWork]`) across the server and clients, Concerto lists disk-authored waves as the baseline, and clicking a wave shells out to bundled `lf goal --tmux` before attaching to the printed tmux handle.

## Assumptions

- Local `tmux` is available.
- The bundled or dev-provided `lf` binary is available to Concerto's GUI process environment.
- Internal lfd stores can take the hard DTO/store cut; old local databases and sessions are disposable.
- Disk-authored waves live under `<repo>/wave/<name>/GOAL.md`.

## Key decisions

- Session names derive from the wave worktree basename: `lf-<repo>-<wave>`.
- `lf goal --tmux` creates deterministic sibling worktrees and reuses an existing stable branch when the branch exists but the worktree is missing.
- Detached tmux stdio is redirected so Concerto does not block waiting for EOF; the macOS launcher captures the first handle line through a temp file and then attaches by name.
- lfd live waves remain an optional overlay. Disk-authored rows still render when lfd is absent, with running state derived from `tmux has-session`.
- Gate polish fixed two multi-repo edges: per-repo DTO `active_run` now comes from that repo's runs, and Concerto wave events update every matching repo state rather than only `repos.first`.
- Stale Swift tests for intentionally deleted native-chat/old-dashboard views were removed with the dead code.

## Not included

- `lfdb` extraction.
- `lf d` / `lf q`.
- lfd executor deletion.
- subscription-backed live status.
- proactive Concerto worktree pre-allocation.
