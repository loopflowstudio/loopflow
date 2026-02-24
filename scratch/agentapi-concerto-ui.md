# Agent API session harness + Concerto transcript (consolidated)

## Scope
This document captures the current state of the agent session work on this branch: Rust provider harnessing, typed session events, and the Concerto chat transcript/session lifecycle updates.

## Current implementation

### Backend (Rust `lfd`)
- Replaced provider-specific adapter flow with a harness layer (`sessions/harness/*`) for `codex` and `claude`.
- Normalized provider output into shared session events before persistence/streaming.
- Kept session/event persistence and SSE replay/live streaming flow aligned with sequence IDs.
- Updated store/routes for provider session ID persistence and single active session behavior per wave run.

### Shared Swift model (`LoopflowCore`)
- Expanded `AgentSessionEvent` coverage to include item lifecycle and diff events.
- Added typed session item/delta models (command/file/message/thought/tool + unknown fallbacks) to mirror Rust event payloads.
- Preserved typed fidelity in shared models; projection/flattening happens in Concerto UI state.

### Concerto chat UX
- Replaced message-only rendering with a unified transcript containing:
  - user/assistant/system messages
  - typed item cards with status + optional detail
- Added stable item-row identity keyed by server `item.id`.
- Added reducer protections:
  - `seq`-based stale/duplicate event filtering
  - best-effort handling of out-of-order item events
- Added bounded detail buffers to prevent unbounded command/tool output growth.
- Added session lifecycle UX:
  - persisted session ID reconnect/replay
  - send disabled during reconnect phase
  - explicit end-session action

## Key decisions worth retaining
1. Keep full typed parity in shared models; flatten only at the view boundary.
2. Use one reducer path for replay + live events.
3. Key transcript item updates by server item ID, not local UUID identity.
4. Keep UI detail memory bounded for long-running sessions.
5. Maintain a single owned stream task in `ChatState` to avoid ghost streams.

## Known limits / follow-up
- Rich viewers (terminal emulation, rendered diffs), turn grouping, and multi-session picker remain out of scope.
- `diff_updated` is plumbed but not deeply visualized.
- Reconnect promotion remains intentionally minimal (first event or short fallback timer).
- Full Concerto UI suite still has environment-sensitive screenshot fragility.

## Verification snapshot
Executed on this branch:
- `cargo fmt --all -- --check` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `tests/e2e/test_smoke.sh` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` ⚠️ fails at `ConcertoUITests/ScreenshotPipelineTests.testCapture`
