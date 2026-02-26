# Verify Concurrent Clients Work

Confirm lfd correctly serves multiple simultaneous Concerto clients on the same instance.

## Context

Stage 03 (multi-client) assumes lfd can handle concurrent connections. This hasn't been verified. If lfd has single-client assumptions (e.g., one WebSocket per repo, per-session singletons), everything else in this stage — suggested action consistency, reconnect resilience, connection UX — is built on a broken foundation.

## What to verify

Two Concerto clients connected to the same lfd simultaneously:

1. **Event streaming**: both clients subscribed to the same wave's events both receive updates
2. **Output streaming**: both clients streaming the same wave's output both receive lines
3. **Chat transcript**: both clients viewing the same session both see messages
4. **Chat input**: sending from either client works without conflict — message appears on both
5. **Suggested actions**: action buttons appear on both clients; tapping on one triggers session event that the other client observes

## What to fix (if broken)

If lfd has single-client assumptions:
- WebSocket connection handling: verify EventService supports multiple subscribers per repo
- Output streaming: verify `/v0/waves/{id}/output/stream` supports concurrent readers
- Session message routing: verify session events broadcast to all connected clients, not just the sender

## Approach

Start with reading lfd's WebSocket and session event handling code to understand the multiplexing model. Then write a test (or extend existing tests) that connects two clients and verifies concurrent behavior.

If lfd already handles this correctly (likely, given broadcast architecture), document the verification and move on. If not, fix the server-side issues before any client work.

## Success criteria

- Two clients see the same wave list and status updates in real time
- Starting a wave on one client shows output on the other
- Chat messages from either client appear on both
- No message loss, no connection conflicts, no panics under concurrent load

## Source

Extracted from `wave/mobile/03-multi-client.md` — "Verify concurrent clients work" section.
