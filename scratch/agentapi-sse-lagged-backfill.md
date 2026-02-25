# SSE Lagged Receiver Backfill

## Problem

`tokio::broadcast` drops messages once a receiver falls behind the 256-event ring buffer. Today `/v0/sessions/{id}/events` handles `RecvError::Lagged(_)` by skipping and continuing, so connected clients can silently miss chunks of a live transcript.

Who benefits: Concerto users following active sessions (especially bursty `text_delta`) and API clients that keep one long-lived SSE connection.

Why now: this is the last obvious gap against the Agent API wave goal **“Reconnect replays persisted events then follows live stream”** and metric **“Session reconnect replays full event history and resumes live streaming without data loss.”** Reconnect is durable (`after_seq`), but in-connection lag is still lossy.

## Approach

Implement in-stream backfill recovery inside `stream_session_events_handler`.

1. Keep startup flow unchanged:
   - subscribe to broadcast
   - replay persisted events from store (`after_seq`)
   - emit one `session.replay_completed` sentinel
2. Extract the live-forwarding loop into a helper so lag behavior is testable without full HTTP setup.
3. In that loop, enforce one monotonic dedup gate everywhere (replay, backfill, live): emit only if `event.seq > last_seq`.
4. On `live_rx.recv()`:
   - `Ok(event)`: emit through dedup gate, advance `last_seq`
   - `Lagged(skipped)`: call `SessionManager::list_events(session_id, Some(last_seq))`, emit returned events in ascending `seq`, advance `last_seq`
   - `Closed`: end stream normally
5. If store backfill fails during lag recovery, terminate this SSE stream (fail closed). Recovery path is explicit reconnect with `after_seq`.
6. Add structured tracing for every lag recovery attempt:
   - `session_id`, `skipped`, `last_seq_before`, `backfilled_count`, and error details on failure.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep current behavior (skip on lag) | Smallest change | Violates reliability goal; causes silent data loss |
| Increase broadcast buffer size (e.g., 4096) | Fewer lag events in normal load | Only delays failure; still lossy at higher burst rates; higher memory footprint |
| Force disconnect on lag, require client reconnect | Correctness via existing replay contract | Adds avoidable disconnect churn; server can recover seamlessly in-stream |

## Key decisions

- **Recover in SSE route, not `SessionManager`:** only the subscriber knows it lagged.
- **Persisted sequence is source of truth:** backfill cursor is `last_seq`, never `skipped` count.
- **One dedup contract across all paths:** no duplicate emissions at replay/live/backfill boundaries.
- **Fail closed on backfill errors:** end stream instead of continuing with unknown gaps.
- **No protocol expansion:** keep existing `session.replay_completed`; no new client event types.

### Wild success

Lag recovery becomes invisible. Under burst output, transcript ordering stays contiguous and duplicate-free; multi-client viewers converge on the same persisted sequence stream.

### Wild failure

We re-open this in six months because backfill emits duplicates or still leaves gaps under racey conditions. Mitigation: one seq gate, helper-level deterministic lag test, and structured lag/backfill tracing to debug real sessions quickly.

## Scope

- In scope:
  - Lagged SSE receiver recovery using persisted store backfill
  - Sequence-based dedup across replay/backfill/live paths
  - Deterministic Rust test that forces lag and verifies contiguous, duplicate-free delivery
  - Tracing around lag/backfill transitions
- Out of scope:
  - Broadcast buffer size changes
  - New SSE protocol events beyond existing sentinel
  - Harness-side lag handling in `spawn_harness_event_bridge`
  - Replay pagination/performance redesign beyond this reliability fix

## Done when

- New Rust test (named with `lagged_receiver_backfill`) forces `RecvError::Lagged` and proves contiguous `seq` delivery with no duplicates.
- Existing no-lag behavior is unchanged (same replay + sentinel + live stream semantics).
- Traces clearly show lag recovery attempts and results.
- Verification passes:
  - `cargo test -p loopflow lagged_receiver_backfill`
