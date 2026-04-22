# Session input gate review

## What was implemented

Added authenticated `/v1/sessions/{id}/input` support for Codex-backed sessions. The endpoint accepts `{"text":"..."}`, rejects empty input, routes through `SessionManager::send_input`, and returns the updated session DTO. Session DTOs now include `input_supported` so clients can disable input for harnesses that cannot handle it.

The existing SSE session event stream remains the read path. The session README documents replay with `after_seq`, the input endpoint, and the capability flag.

## Key choices

- Reused the harness `send_input` abstraction instead of exposing `steer` vs `new turn` at HTTP level.
- Kept input Codex-only for v1; Claude and OpenCode report `input_supported: false`, and the server rejects `/input` before looking up a runtime.
- Mounted the same API router under `/v1` while preserving `/v0` for existing clients and scripts.
- Kept a serde alias for legacy `content` request bodies while moving first-party clients to `text`.
- Strengthened the round-trip test during gate so the done-when path now uses real HTTP requests and SSE replay instead of only calling `SessionManager` directly.

## How it fits together

`SessionDto` computes `input_supported` from the harness kind. `send_session_input_handler` validates the body, calls `SessionManager::send_input`, and maps unsupported harnesses to a clear 400 response. `SessionManager::send_input` checks active state and harness capability before locking the live harness runtime, so unsupported harnesses fail deterministically even if no runtime is present.

## Risks and bottlenecks

- Concurrent clients still share the existing harness send path. A tiny race can still surface as `turn already in progress`; clients should retry.
- Claude support is intentionally absent until the harness has a stable bidirectional control protocol.
- `/v1` currently mirrors the whole `/v0` API router. That is simple and matches this branch's needs, but future breaking changes will need route-level versioning discipline.

## What's not included

- Tool approvals or safety gating.
- Claude or OpenCode input support.
- New session event variants.
- iPhone Concerto UI.
- WebSocket replacement for SSE.

## Validation

- `cargo fmt --check`
- `cargo test -p loopflow --test session_input_round_trip`
- `cargo test -p loopflow session_input`
- `uv run pytest python/tests/test_client.py python/tests/test_models.py -q`
- `swift test --package-path swift`
- `cargo clippy -p loopflow -- -D warnings`
