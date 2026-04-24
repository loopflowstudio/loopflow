---
asana_id: '1214269992270171'
---
# Chat session API

**Finish line:** lfd exposes a complete native-chat backend: typed activity events (content), bidi input path (write), resumable WebSocket stream (resilience). Desktop and mobile chat UIs consume this one surface. A conversation started on one device continues seamlessly on another, survives network drops, and renders typed activity (edits, shell, search) differently from plain text.

## Context

Currently lfd streams raw text and a one-shot WebSocket snapshot. Native chat (desktop + mobile) needs three things:

- **Typed events** — "agent edited `src/api/routes.rs`" vs "agent ran `cargo test`" render differently. Normalized across providers.
- **Bidi input** — a second device joins a running session and sends messages in, mid-turn or new-turn, routed through the harness's `send_input`
- **Resumable stream** — client sends "give me everything after seq N," server delivers exactly that. Phones dropping connections stop losing events.

## Daily experience

Chat with an agent on desktop about a design decision. Walk to lunch. Open phone — conversation is there; send a new reply while waiting for food. Come back, desktop picked up where phone left off. Throughout: code edits appear as diff cards, shell runs as structured panels, searches as collapsible lists.

## Done when

- Three lfd additions shipped: `ActivityEvent` stream, session-input endpoint, main-WebSocket `after_seq` cursor
- Desktop chat UI consumes typed events with distinct rendering
- Mobile chat UI uses the same API
- Network drop + reconnect loses zero events
- Mid-turn input from a second device interrupts cleanly
