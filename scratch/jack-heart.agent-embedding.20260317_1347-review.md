# Review: jack-heart.agent-embedding.20260317_1347

## What was implemented

Wave workspace routing and embedded terminal infrastructure for Concerto.

- **WaveWorkspaceView**: new primary container for selected waves. Routes to native work view by default, embedded terminal tab when a terminal session exists.
- **TerminalWorkspaceView**: Ghostty-backed terminal surface that launches `lf <step>` in the wave's worktree. Sessions track lifecycle (pending → attached → running → succeeded/failed). Exit code 0 resumes the wave; non-zero fails the run.
- **Attention kind alignment**: Swift models now decode the backend attention kinds (`design_review`, `code_review`, `calibration`, `queue_failure`, `step_failure`) instead of placeholder values.
- **Terminal session backend**: full CRUD routes, SQLite/Postgres store, migration, event broadcasting, and wave executor integration.
- **ContentView routing**: selected wave → WaveWorkspaceView, no selection → AttentionQueueView. Terminal takeover removed.

## Key choices

- Terminal is additive, not a takeover. The tab bar only appears when a terminal session exists for the selected wave. Native chat/session UI remains the default.
- Attention kinds are mapped 1:1 from backend rather than collapsed into semantic buckets. Direct mapping is simpler and avoids lossy translation.
- `GhosttyManager` is accessed via `@ObservedObject` (externally owned singleton), not `@StateObject` (view-owned). Fixed during gate.
- ISO8601 date formatters cached as static lets to avoid per-event allocation.

## How it fits together

```
ContentView
  ├── no wave selected → AttentionQueueView (repo-wide)
  └── wave selected → WaveWorkspaceView
       ├── Work tab (default) → WaveDetailPanel
       └── Terminal tab (when session exists) → TerminalWorkspaceView
            └── SessionTerminalSurface → GhosttyTerminalView
```

Backend: `lfd` creates terminal sessions via HTTP routes → stored in SQLite/Postgres → events broadcast over WebSocket → Swift `TerminalWorkspaceStore` tracks state → `RepoState` bridges selection.

## Risks and bottlenecks

- Ghostty library linkage is build-environment sensitive. The `GhosttyTerminalView` depends on the Ghostty C library being available at link time.
- `ConcertoUITests` runner fails to bootstrap on macOS 26.0.1 — pre-existing issue, not introduced by this branch.
- Terminal session cleanup depends on the Ghostty `onSessionClosed` callback firing reliably. If the process is killed externally (SIGKILL), the session may stay in `running` state.

## What's not included

- Multi-wave command grid (future milestone per design doc)
- tmux-like pane management
- Layout persistence
- Wave config/settings redesign (deferred — WaveDetailPanel still handles that path)

## Validation

### Automated checks

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | 1 pre-existing failure (`wave_rename_renames_branch`) |
| `uv run pytest python/tests/` | 113 passed |
| `swift test --package-path swift` | 242 passed |
| `tests/e2e/test_smoke.sh` | pass |
| `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | 16 passed |

### Gate fixes applied

- `GhosttyTerminalView`: replaced hardcoded byte count `6` with `clearCmd.utf8.count`
- `TerminalWorkspaceView`, `InteractiveSessionView`: `@StateObject` → `@ObservedObject` for singleton
- `InteractiveSessionView`: action methods now use injected `ghosttyManager` instead of `GhosttyManager.shared` directly
- `LocalEventService`: cached ISO8601 formatters as `nonisolated(unsafe) static let`
- `WaveWorkspaceView`: added `ProgressView` placeholder when terminal tab is selected but session hasn't loaded yet

### Manual product check

Run `uv run python scripts/concerto-dev.py run-debug`, start two waves with interactive steps, verify:
1. Selected wave opens work surface, not terminal takeover
2. Terminal tab appears when session exists, native chat is default
3. Terminal exit 0 resumes wave, non-zero fails run
4. No-selection shows attention queue
5. Attention items render with correct kinds
