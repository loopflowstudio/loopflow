# Gate Review — concurrent client reliability regression coverage

## What was implemented

- Added `tests/e2e/test_concurrent_clients.py` with five e2e checks for two simultaneous clients against one `lfd`:
  - dual WebSocket wave event fanout
  - dual wave log streaming
  - dual SSE session event fanout
  - chat input fanout visibility across both subscribers
  - `suggested_actions` SSE payload parseability in the Python client
- Wired the new concurrent-client suite into CI by extending the `e2e-smoke` pytest invocation in `.github/workflows/ci.yml`.
- Updated Stage 03 wave tracking doc to mark concurrent-client verification as ingested to `scratch/mobile-verify-concurrent-clients.md`.
- Updated `TESTING.md` so local and CI command docs match the new e2e invocation.

## Key choices

- **No lfd implementation changes**: this branch verifies existing broadcast/replay architecture rather than modifying runtime behavior.
- **Threaded SSE collectors**: used blocking `Client.stream_session_events()` in per-client threads to keep test logic simple and close to real client usage.
- **Compatibility-safe WebSocket auth headers**: runtime detection of `websockets.connect()` header parameter name (`additional_headers` vs `extra_headers`) avoids version coupling.
- **Chat input assertion hardening**: if `send_session_input()` succeeds, both subscribers must observe downstream input-related events; fallback path is only used for explicit send failures.

## How it fits together

The suite starts one hermetic `lfd` via existing `lfd_runtime` fixtures, then connects two independent clients/subscribers to the same backend endpoints (`/ws`, `/logs`, `/sessions/{id}/events`). Each test triggers one server-side action and asserts both clients observe the same broadcasted outcome. CI now runs this suite in the existing `e2e-smoke` job so concurrent regressions fail the same gate as API smoke coverage.

## Risks and bottlenecks

- Session/chat assertions still depend on harness availability and startup behavior; the test handles expected send failures, but this area remains the most environment-sensitive path.
- Time-based coordination (`10s` deadlines, short sleeps before run) is pragmatic but can become flaky under extreme CI load.
- The suite validates fanout correctness and parseability, not high-throughput lag/recovery behavior under sustained event pressure.

## What's not included

- No server-side concurrency rework in `lfd` (EventHub/OutputHub/SessionManager internals unchanged).
- No iOS/macOS UI behavior changes for action clearing or reconnect; this branch is backend/API regression coverage.
- No load/performance benchmarking for event lag or backpressure scenarios.

## Validation run for this gate pass

- `uv run pytest tests/e2e/test_concurrent_clients.py -v`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
