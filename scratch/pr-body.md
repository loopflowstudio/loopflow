## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
```

For the product flow:

```bash
uv run python scripts/concerto-dev.py run-debug
```

Start two local waves that pause on interactive steps. Verify:

- selecting a wave opens a work surface (not a terminal takeover)
- one Ghostty-backed terminal tab per interactive wave, available alongside native chat
- sidebar with wave metadata, queue pressure, recent run/PR context, and quick actions
- terminal exit 0 resumes the wave in `lfd`; non-zero exits fail the run
- no-wave-selected shows the repo-wide attention queue with real backend data

## Intent

Reframe the macOS UI around a **wave workspace** instead of a terminal takeover. Embedded terminals are additive — they surface as an optional tab when a terminal session exists for the selected wave. Native chat/TUI sessions remain the default interactive surface.

Backend adds terminal session CRUD (SQLite + Postgres), lifecycle events, and wave executor integration so `lfd` can track and resume sessions by exit code.

Swift attention models now decode backend attention kinds (`design_review`, `code_review`, `calibration`, `queue_failure`, `step_failure`) directly instead of using placeholder values.

## Assumptions

- Ghostty C library is available at Concerto link time
- `lfd` WebSocket broadcasts terminal session events that Concerto subscribes to
- Wave executor creates terminal sessions when a step requires interactive input

## Key decisions

- **Terminal as tab, not takeover**: tab bar only appears when a terminal session exists. No UI change for waves without terminal sessions.
- **1:1 attention kind mapping**: direct mapping from Rust enum variants to Swift enum cases. No semantic collapsing — simpler and avoids lossy translation.
- **`@ObservedObject` for singleton**: GhosttyManager is externally owned; views observe but don't own it.

## Not included

- Multi-wave command grid (future milestone)
- tmux-like pane management or layout persistence
- Wave config/settings redesign
