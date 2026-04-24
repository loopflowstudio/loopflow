---
asana_id: '1214269992270171'
---
# Chat session API

**Finish line:** lfd exposes a complete native-chat backend: typed activity events, bidi input, and a resumable stream. Desktop chat uses it first; later clients can join the same session model without inventing a second protocol.

## Context

Today lfd streams raw text and a one-shot WebSocket snapshot. Native chat needs three things:

- **Typed events** — "agent edited `src/api/routes.rs`" vs "agent ran `cargo test`" render differently. Normalized across providers.
- **Bidi input** — a second client can join a running session and send messages in, mid-turn or new-turn, routed through the harness's `send_input`
- **Resumable stream** — a client asks for everything after seq `N` and gets exactly that. Network drops stop losing events.

## Daily experience

Talk to an agent on desktop about a design decision. The transcript renders structured edits, shell runs, and searches instead of flattening them into plain text. If another client joins later, it picks up from the same session state instead of starting over.

## Done when

- Three lfd additions ship: `ActivityEvent` stream, session-input endpoint, main-WebSocket `after_seq` cursor
- Desktop chat UI consumes typed events with distinct rendering
- Another client can attach later without losing events or inventing a separate sync path
- Network drop + reconnect loses zero events
- Mid-turn input from a second client interrupts cleanly
