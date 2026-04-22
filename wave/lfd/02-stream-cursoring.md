# 04: Stream Cursoring

**Finish line:** WebSocket clients can disconnect and reconnect without losing events. The client sends "give me everything after sequence N" and lfd delivers exactly that.

## Context

The existing WebSocket sends a full snapshot on connect (`connected` event) and then streams mutations. If the connection drops, the client refetches everything. This works on a stable desktop connection. It doesn't work on a phone where connections drop every time you walk through a door.

Session SSE already has `after_seq`. The main WebSocket doesn't.

## What to build

**Sequence numbers on events.** Every event broadcast on the WebSocket gets a monotonically increasing sequence number. The counter is global to the lfd instance (not per-wave or per-session).

**Reconnect with cursor.** Client sends `{ resume_after: N }` in the hello/reconnect message. lfd replays all events with seq > N from a buffer, then switches to live streaming.

**Server-side buffer.** lfd holds the last K events (or last T seconds) in a ring buffer. If the client's cursor falls outside the buffer, lfd sends a full snapshot instead of a replay. The client handles both paths.

## Constraints

- Start simple: monotonic counter, fixed-size ring buffer, full-snapshot fallback
- Don't add epochs, gap detection, or compaction until real mobile testing shows they're needed
- Buffer size should be configurable (default: 1000 events or 5 minutes, whichever is larger)
- Sequence numbers must survive lfd restarts (persist the counter to the database)

## Done when

- Every WebSocket event has a `seq` field
- A client that disconnects for 30 seconds and reconnects receives only the missed events
- A client that disconnects for longer than the buffer window receives a fresh snapshot
- Concerto desktop handles both reconnect paths without user-visible glitch
