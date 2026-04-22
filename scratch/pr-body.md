## Try it!

```bash
cargo test -p loopflow --test session_input_round_trip
```

The test starts a live `lfd` HTTP router with fake Codex/Claude harnesses, creates a Codex session through `/v1/sessions`, posts input through `/v1/sessions/{id}/input`, observes the running turn through SSE, reconnects with `after_seq`, and verifies Claude input is rejected with a clear unsupported-harness error.

Other checks run during gate:

```bash
cargo fmt --check
cargo test -p loopflow session_input
uv run pytest python/tests/test_client.py python/tests/test_models.py -q
swift test --package-path swift
cargo clippy -p loopflow -- -D warnings
```

## Intent

Let a second device continue a running agent conversation without a terminal. Reading stays on the existing session SSE stream; writing is one authenticated `/v1/sessions/{id}/input` endpoint that reuses the harness `send_input` path.

## Assumptions

- Codex is the only harness with a stable enough live control path for v1.
- Tools continue to auto-approve; this endpoint is conversation continuity, not approval gating.
- Clients use `input_supported` to decide whether to show or disable the composer.

## Key decisions

- Expose a single `{"text":"..."}` request shape and let the harness decide whether input steers a running turn or starts a new one.
- Return `input_supported: true` only for Codex sessions; Claude/OpenCode return `false` and reject POST input.
- Keep `/v0` working while adding `/v1` routes for the new mobile-facing API surface.
- Update Python, Swift, and validation scripts to send `text` instead of `content`.

## Not included

- Approval APIs, pending prompt tables, TTLs, or decision enums.
- Claude/OpenCode input support.
- iPhone client UI.
- WebSocket migration for session events.
