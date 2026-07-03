## Try it!

```bash
cargo build --bin lf
target/debug/lf goal concerto --tmux
tmux attach-session -t lf-loopflow-concerto
```

Then launch Concerto against this repo and kill lfd; disk-authored waves should still render, and selecting one should launch/attach through `lf goal <wave> --tmux`.

Validation run during gate:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
uv run python -m pytest python/tests/
swift test --package-path swift
uv run python scripts/check_swift_multiplatform_boundaries.py
uv run python -m pytest tests/regression/ -v
uv run python -m pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
tests/e2e/test_smoke.sh
cd website && uv run python dev.py test
docker version && cargo test -p loopflow docker_ -- --nocapture
```

Also ran the Xcode project path. The full non-UI Xcode suite built but failed locally on a pre-existing `KeyboardRouterTests` chord-timeout scheduling issue; `swift test --package-path swift` passed the same suite, and `xcodebuild ... -only-testing:ConcertoTests/KeyboardRouterTests` passed in isolation (`17 passed`).

## Intent

Make Concerto's basic wave UX work without lfd in the launch/attach path. Waves are now repo-backed (`repos: [RepoWork]`) across the server and clients, Concerto lists disk-authored waves as the baseline, and clicking a wave shells out to bundled `lf goal --tmux` before attaching to the printed tmux handle.

## Assumptions

- Local `tmux` is available.
- The bundled or dev-provided `lf` binary is on the GUI process path Concerto uses.
- Internal lfd stores can take the hard DTO/store cut; old local databases and sessions are disposable.
- Disk-authored waves live under `<repo>/wave/<name>/GOAL.md`.

## Key decisions

- Session names derive from the wave worktree basename: `lf-<repo>-<wave>`.
- `lf goal --tmux` creates deterministic sibling worktrees and reuses an existing stable branch when the branch exists but the worktree is missing.
- Detached tmux stdio is redirected so Concerto does not block waiting for EOF; the macOS launcher captures the first handle line through a temp file and then attaches by name.
- lfd live waves remain an optional overlay. Disk-authored rows still render when lfd is absent, with running state derived from `tmux has-session`.
- Stale Swift tests for intentionally deleted native-chat/old-dashboard views were removed with the dead code.

## Not included

- `lfdb` extraction.
- `lf d` / `lf q`.
- lfd executor deletion.
- subscription-backed live status.
- proactive Concerto worktree pre-allocation.
