# SSE Lagged Receiver Backfill

## Problem

`tokio::broadcast` drops old messages when an SSE receiver falls behind the 256-event buffer. Today `/v0/sessions/{id}/events` handles `RecvError::Lagged(_)` by skipping and continuing, which creates silent gaps for connected clients.

Who benefits: Concerto users watching active sessions (especially bursty `text_delta` output) and any API client that keeps one long-lived SSE connection.

Why now: This blocks the hardening goal of “replay full event history and resume live streaming without data loss.” Reconnect (`after_seq`) is durable, but during-connection lag is still lossy.

## Approach

Implement **in-stream backfill recovery** in `stream_session_events_handler`.

1. Keep current startup flow:
   - subscribe to live broadcast
   - replay persisted events from store (`after_seq`)
   - emit one `session.replay_completed` sentinel
2. In the live loop, when `live_rx.recv()` returns `Lagged(skipped)`:
   - query store for events `seq > last_seq` via `SessionManager::list_events(session_id, Some(last_seq))`
   - emit backfilled events in ascending `seq`
   - update `last_seq` per emitted event
   - return to broadcast receive loop
3. Enforce one dedup gate everywhere (replay, backfill, live): emit only when `event.seq > last_seq`.
4. If store backfill fails, terminate this SSE stream (fail closed, not lossy-continue). Client reconnect with `after_seq` remains the recovery path.
5. Add structured tracing for lag recovery (`session_id`, `skipped`, `last_seq_before`, `backfilled_count`).

Implementation detail for testability: extract the live-forwarding loop into a small helper so lag/backfill behavior can be tested without full HTTP server setup.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep current behavior (skip on lag) | Simplest code path | Violates reliability goal; causes silent data loss |
| Increase broadcast buffer size (e.g., 4096) | Fewer lag events under normal load | Delays failure but does not remove it; memory grows; still lossy at higher burst sizes |
| Drop connection on lag and require reconnect | Correctness via existing `after_seq` replay | Adds visible disconnect churn; avoidable UX hit when server can recover in-stream |

## Key decisions

- **Recover in the SSE route, not SessionManager**: only the SSE subscriber knows it lagged.
- **Store sequence is source of truth**: backfill is cursored by `last_seq`, not by skipped count from broadcast.
- **Monotonic emission contract**: never emit `seq <= last_seq`; this guarantees no duplicates across replay/backfill/live boundaries.
- **Fail closed on backfill errors**: better to end stream than silently continue with gaps.

### Wild success

Users never notice lag recovery. Even under burst output, transcripts remain contiguous and ordered; multi-client viewing stays consistent because all clients converge on the same persisted sequence stream.

### Wild failure

Six months later we rip it out because lag handling became flaky due to duplicate/gap edge cases and untestable async logic. Mitigation in this design: single monotonic seq gate, helper-level tests that force lag deterministically, and explicit tracing around every lag/backfill transition.

## Scope

- In scope:
  - Lagged SSE receiver recovery using persisted store backfill
  - Seq-based dedup across replay/backfill/live
  - Test that forces `Lagged` and verifies contiguous delivery
  - Tracing for lag/backfill transitions
- Out of scope:
  - Changing broadcast buffer sizes
  - New client protocol events (existing `session.replay_completed` unchanged)
  - Harness-side lag in `spawn_harness_event_bridge`
  - Replay pagination/performance redesign beyond this reliability fix

## Done when

- New Rust test simulates lag (`Lagged`) and verifies the client receives a gap-free, duplicate-free contiguous seq stream.
- Existing fast-path behavior (no lag) remains unchanged.
- Verification command passes:
  - `cargo test -p loopflow lagged_receiver_backfill`
