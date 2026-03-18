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

- selecting a wave opens a work surface instead of a terminal takeover
- the native work/session view stays the default tab
- a Ghostty-backed Terminal tab appears only when that wave has an active terminal session
- the terminal sidebar shows wave metadata, queue pressure, recent run/PR context, and quick actions
- terminal exit 0 resumes the wave in `lfd`; non-zero exits fail the run
- no-wave-selected still lands on the repo-wide attention queue with live backend data

Automated validation from this gate:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (113 passed)
- `swift test --package-path swift` ✅ (243 passed)
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (smoke pass + 16 passed)
- `xcodebuild test ...` built the app and unit suites, but the UI runner hung before establishing connection in this no-rendering environment

## Intent

Turn the selected-wave experience into a **workspace-first** UI. Embedded terminals should support interactive work without replacing the native wave detail/session surface, and terminal lifecycle should be durable enough for `lfd` to resume or fail waiting runs from terminal exit codes.

## Assumptions

- Ghostty's C library is available when Concerto links/runs terminal views.
- `lfd` broadcasts terminal-session lifecycle events over the existing websocket stream.
- Interactive wave steps create terminal-session records server-side before the client renders them.
- Full macOS UI automation needs a normal logged-in GUI session; this gate ran headless with no rendering environment.

## Key decisions

- **Terminal as an additive tab**: `WaveWorkspaceView` defaults to the native work view and only exposes a Terminal tab when the selected wave has an active terminal session.
- **Server-owned terminal state**: terminal sessions live in `lfd` storage and APIs, not only in the client, so lifecycle state survives reconnects/restarts.
- **Typed attention parity**: Swift decodes backend attention kinds directly instead of collapsing them into placeholders.
- **Separate terminal workspace store**: terminal-session selection/order is isolated from chat/session state so the existing interactive-session path stays intact.

## Not included

- Remote terminal transport
- Multi-wave terminal grids or pane management
- Layout persistence
- Wave settings/config redesign
