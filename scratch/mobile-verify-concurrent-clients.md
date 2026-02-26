# Verify Concurrent Clients Work

## Problem

Stage 03 (multi-client) assumes lfd can handle concurrent connections. Every feature in that stage — suggested action consistency, reconnect resilience, connection UX — depends on this being true. If lfd has single-client assumptions, everything built on top is broken.

Two Concerto clients (Mac + iPhone) must see the same waves, same sessions, same output, in real time. We need to prove this works and catch regressions.

## Approach

**No lfd code changes.** Code review confirms the architecture already supports concurrent clients:

- **EventHub** (`rust/loopflow/src/lfd/events.rs`): `tokio::sync::broadcast` channel (cap 1024). Each `subscribe()` call returns an independent receiver. Multiple WebSocket connections each get their own receiver.
- **OutputHub** (`rust/loopflow/src/lfd/output.rs`): `tokio::sync::broadcast` (cap 2048). Same pattern. The `/v0/waves/{id}/logs` endpoint does replay-then-follow per request — each client streams independently.
- **Session events** (`rust/loopflow/src/lfd/sessions/mod.rs`): Per-session `broadcast::channel(256)`. `subscribe()` gives each caller an independent receiver. The SSE endpoint (`/v0/sessions/{id}/events`) implements replay-then-follow with lag backfill from the store — slow clients recover without losing events.
- **Chat input** (`send_input`): Serialized via `Mutex<Harness>`. Concurrent sends return 409 `TurnAlreadyInProgress` — correct behavior, not a bug. The response event broadcasts to all session subscribers.
- **No singletons**: `HttpState`, `SessionManager`, `EventHub`, `OutputHub` are all `Clone` wrappers around `Arc`. No connection "owns" shared state.

**Write a Python e2e test** (`tests/e2e/test_concurrent_clients.py`) that connects two clients to the same lfd and verifies all five concurrent behaviors. This becomes a regression guard — if anyone introduces a single-client assumption, the test catches it.

### Test structure

One test file, five test functions, using the existing `lfd_runtime` pytest fixture:

1. **`test_both_ws_clients_receive_wave_events`** — Two WebSocket connections. Create a wave via HTTP from one client. Both WS clients receive the `wave_created` event.

2. **`test_both_clients_stream_output`** — Two HTTP clients call `GET /v0/waves/{id}/logs` concurrently (via threads). Start a wave. Both clients receive the same output lines.

3. **`test_both_clients_receive_session_events`** — Create a session. Two SSE streams via `client.stream_session_events()` in separate threads. Both receive the session's events (at minimum `StatusChanged`).

4. **`test_chat_input_from_either_client_visible_to_both`** — Two SSE subscribers on the same session. Send input from client A. Both A and B see the resulting session events. (Requires a real or mock harness that echoes — if no harness is available in the test environment, verify the `InputReceived` event broadcasts to both.)

5. **`test_suggested_actions_broadcast`** — Suggested actions are just a `SessionEvent::SuggestedActions` variant. If test 3 passes (session events broadcast to all subscribers), suggested actions are covered. No separate test needed — but assert the event type string `"suggested_actions"` is parseable by the client.

### Test mechanics

- Use the existing `lfd_runtime` fixture from `tests/e2e/conftest.py` — hermetic lfd with isolated temp dir.
- Create two `Client` instances from `python/loopflow/client.py` pointing at the same `base_url`/`token`.
- WebSocket tests use the `websockets` library (already a dev dependency) with `asyncio.run()`.
- SSE streaming tests use `threading.Thread` to run two `stream_session_events()` consumers concurrently, collecting events into thread-safe lists.
- Timeout: 10 seconds per test. If events don't arrive within 10s, fail — don't hang.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Rust integration test | Closer to the metal, faster execution | Existing e2e patterns are Python; mixing would fragment test infrastructure. Python tests are more readable for verifying HTTP/WS/SSE behavior. |
| Manual script in `scripts/` | One-off verification, no CI regression guard | We need this to run in CI forever. A test is better than a script. |
| Code review only, no test | Fastest "verification" | Proves nothing about runtime behavior. Race conditions and integration bugs hide from code review. |
| Fix WS lag handling | Would close the one gap (BroadcastStream silently drops lagged events on WS) | Separate concern. Lag only matters under extreme event throughput — not a correctness issue for Concerto. File as a separate backlog item if needed. |

## Key decisions

**No lfd changes.** The architecture is already correct. Adding code "just in case" would be churn. The test is the deliverable.

**Python e2e test, not Rust.** The existing e2e test infrastructure is Python (`tests/e2e/test_api_smoke.py`, `conftest.py`, `LfdRuntime`). Following that pattern keeps the test suite coherent. The `Client` class already has `stream_session_events()` with `after_seq` support — reuse it.

**Thread-based concurrency for SSE.** `stream_session_events()` is a blocking generator. Simplest approach: run each consumer in a `threading.Thread`, collect events into a `list` (protected by the GIL — no lock needed for append). `asyncio` would work but adds complexity for no benefit.

**No mock harness.** Tests 1-3 don't require a session agent to be running — they verify broadcast mechanics using wave CRUD events and session lifecycle events. Test 4 (chat input) may need a running harness to produce response events. If the test environment doesn't have Claude/Codex available, test 4 can verify that `InputReceived` (or the 409 "no active turn") behavior is consistent across both subscribers. The broadcast correctness is the same regardless of whether a harness processes the input.

**Suggested actions don't need a separate test.** They're a `SessionEvent` variant. If session event broadcast works (test 3), suggested actions work. The serialization format is already tested elsewhere.

## Scope

- **In scope**: Python e2e test proving concurrent client behavior. Five test functions covering WebSocket events, output streaming, session events, chat input, and suggested action broadcast. CI integration (runs with existing `e2e-smoke` job).
- **Out of scope**: WS lag detection/recovery (separate concern). Client-side UI changes. lfd code changes. Suggested action clearing logic (that's a Concerto client concern, covered elsewhere in Stage 03).

## Done when

```bash
uv run pytest tests/e2e/test_concurrent_clients.py -v
```

All five tests pass. Two clients connected to the same lfd simultaneously see the same events, output, and session state. The test runs in CI as part of the `e2e-smoke` job.

**Wave goals advanced:** "Both see the same wave list and status updates in real time" and "Chat transcript visible on both devices, messages from either device appear on both" from Stage 03 done-when criteria.
