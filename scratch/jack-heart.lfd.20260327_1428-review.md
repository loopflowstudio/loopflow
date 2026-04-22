# Session input gate review

## What was implemented

Added authenticated `/v0/sessions/{id}/input` support for Codex-backed sessions. The endpoint accepts `{"text":"..."}`, rejects empty input, routes through `SessionManager::send_input`, and returns the updated session DTO. Session DTOs now include `input_supported` so clients can disable input for harnesses that cannot handle it.

The existing SSE session event stream remains the read path. The session README documents replay with `after_seq`, the input endpoint, and the capability flag.

A code-review pass on the branch tightened several decisions: versioning collapsed to `/v0` only, harness capability moved onto `HarnessKind` as a compile-exhaustive method, the Swift `inputSupported` defaults removed in favor of a pessimistic-until-server-responds model, and a repo-wide STYLE.md rule against defaults on wire DTOs was introduced and exercised via round-trip fixture tests in Rust, Python, and Swift.

## Key choices

- Reused the harness `send_input` abstraction instead of exposing `steer` vs `new turn` at HTTP level.
- Kept input Codex-only; Claude and OpenCode report `input_supported: false`, and the server rejects `/input` before looking up a runtime.
- Capability lives on `HarnessKind::input_supported(self)` so adding a harness variant is a build error until the question is answered.
- Stayed on `/v0`. No parallel version mount; mobile and desktop talk the same API surface.
- Dropped the `content` serde alias — first-party clients moved to `text` in the same PR, no legacy callers to support.
- Swift DTO (`AgentSession`) has no default for `inputSupported`; `SessionState` starts `false` and lets `canSend` treat the no-session-yet case as unknown-permit. Once a session exists, an unsupported harness blocks send with a surfaced system message.
- Strengthened the round-trip test so the done-when path uses real HTTP requests and SSE replay instead of only calling `SessionManager` directly.

## How it fits together

`SessionDto` computes `input_supported` via `HarnessKind::parse(&session.harness).map(HarnessKind::input_supported).unwrap_or(false)`. `send_session_input_handler` validates the body, calls `SessionManager::send_input`, and maps unsupported harnesses to a clear 400 response. `SessionManager::send_input` checks active state and harness capability before locking the live harness runtime, so unsupported harnesses fail deterministically even if no runtime is present.

Fixture tests under `tests/fixtures/dto/` hold canonical wire bytes for `Session` DTOs (one Codex / input supported, one Claude / input denied). Rust, Python, and Swift each parse the fixtures and assert equivalent values — any silent divergence between the three mirrors becomes a test failure.

## Risks and bottlenecks

- Concurrent clients still share the existing harness send path. A tiny race can still surface as `turn already in progress`; clients should retry.
- Claude support is intentionally absent until the harness has a stable bidirectional control protocol.
- Python `SessionConfig` is thinner than the Rust DTO (no `step`, `repo_root`, `directions`, etc.). Pydantic silently drops the extras; the fixture test currently asserts only on fields Python models. Follow-up: align `SessionConfig` across languages.

## What's not included

- Tool approvals or safety gating.
- Claude or OpenCode input support.
- New session event variants.
- iPhone Concerto UI.
- WebSocket replacement for SSE.
- Generated Swift DTOs — declined for now in favor of the STYLE rule + fixture tests.

## Validation

- `cargo fmt --check`
- `cargo test -p loopflow --test session_input_round_trip`
- `cargo test -p loopflow --test dto_fixtures`
- `cargo test -p loopflow session_input`
- `uv run pytest python/tests/test_dto_fixtures.py python/tests/test_client.py python/tests/test_models.py -q`
- `swift test --package-path swift`
- `cargo clippy -p loopflow -- -D warnings`
