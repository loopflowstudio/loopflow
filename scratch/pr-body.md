## Try it!

```bash
cargo test -p loopflow --test session_input_round_trip
```

The test starts a live `lfd` HTTP router with fake Codex/Claude harnesses, creates a Codex session through `/v0/sessions`, posts input through `/v0/sessions/{id}/input`, observes the running turn through SSE, reconnects with `after_seq`, and verifies Claude input is rejected with a clear unsupported-harness error.

Other checks run during gate:

```bash
cargo fmt --check
cargo test -p loopflow --test dto_fixtures
cargo test -p loopflow session_input
uv run pytest python/tests/test_dto_fixtures.py python/tests/test_client.py python/tests/test_models.py -q
swift test --package-path swift
cargo clippy -p loopflow -- -D warnings
```

## Intent

Let a second device continue a running agent conversation without a terminal. Reading stays on the existing session SSE stream; writing is one authenticated `/v0/sessions/{id}/input` endpoint that reuses the harness `send_input` path.

## Assumptions

- Codex is the only harness with a stable enough live control path for v1 of this feature.
- Tools continue to auto-approve; this endpoint is conversation continuity, not approval gating.
- Clients use `input_supported` to decide whether to show or disable the composer.

## Key decisions

- Expose a single `{"text":"..."}` request shape and let the harness decide whether input steers a running turn or starts a new one.
- Return `input_supported: true` only for Codex sessions; Claude/OpenCode return `false` and reject POST input.
- Capability lives on `HarnessKind::input_supported(self)` as a compile-exhaustive match — adding a harness variant is a build error until the question is answered.
- Stay on `/v0`; no version split.
- Swift DTO drops its `inputSupported = true` default; `SessionState` starts pessimistic and lets the first session response (join or create) set the real value.
- New STYLE.md rule — no defaults on wire DTOs — with fixture round-trip tests under `tests/fixtures/dto/` exercised from Rust, Python, and Swift.

## Not included

- Approval APIs, pending prompt tables, TTLs, or decision enums.
- Claude/OpenCode input support.
- iPhone client UI.
- WebSocket migration for session events.
- Python `SessionConfig` parity — its model is thinner than Rust's; the fixture test surfaces this, follow-up to align.
