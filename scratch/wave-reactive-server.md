---
requires: none
produces: the wave runtime design
---
A Wave is a long-lived reactive server: it listens to its subagents' progress and
to the user's messages, and acts as each arrives. Not a loop — an event server.

## The core

`lf wave <name>` starts a server that stays up until you stop it. It is not
driven by a loop that polls; it **reacts to events**. Two event sources feed it,
and they are independent — one firing never blocks the other:

```
                ┌─ subagent progress events ──┐
  Wave server ──┤                             ├──▶ react: record a Turn, update MEMORY,
                └─ user messages ─────────────┘           spawn/steer a subagent, reply
```

The independence is the whole point. Progress work and chat are just two streams
into the same reactive server; the user gets answered immediately, from current
state, while subagents grind in the background. Nothing hands a user message to
"the next pass" — the server reacts to it the moment it arrives.

## Primitives

- **Wave server** — the always-on process. Owns the state below, holds the event
  inputs open, and reacts. This is the thing `lf wave` runs.
- **Subagents** — the work. The server spawns them (bounded runs) and listens to
  their streamed output as an event source. Progress = subagent activity.
- **Thread** — `conversation: [Turn]`. One timeline the user sees, fed by three
  producers: the user (their messages), the server's replies, and progress
  narration. This is what Concerto renders.
- **MEMORY** — `MEMORY.md`. Durable shared brain. The server reads it to answer
  and reacts into it as things change.

## What the server does on each event

- **Subagent emits progress** → map its stream into the thread (via the restored
  harness — codex first), and fold durable facts into MEMORY.
- **User sends a message** → answer from MEMORY + current progress state, and act
  if asked (steer a subagent, note a priority in MEMORY, spawn work).

The harness (restored broadly in #791 — all vendors, conformance-tested) is the
subagent→Turn mapper. That part of #791 we keep; the file-based server
(`MAILBOX.md`, NDJSON sink) we replace with this reactive model. No files as IPC:
the inbox, the thread, and progress are in-process structures.

## The wire surface (a thin view over server state)

- `GET /conversation` (+ SSE `/conversation/stream`) → the thread, live.
- `POST /messages {text}` → deliver a user event to the server.
- `GET /health` → liveness + status.

Concerto finds a running wave's server (discovery TBD — a small registry or a
per-wave endpoint file that only carries host:port, not logic) and renders the
thread + a composer.

## Build order (from the server out)

1. The wave server + its two event inputs (subagent stream, user messages) and
   the in-process state (thread, MEMORY handle).
2. Subagent supervision — spawn a bounded run, stream it in as progress events.
3. React: progress → Turns (harness) + MEMORY; user message → reply.
4. The wire surface (conversation + messages), then Concerto.

## Decisions (Jack)

- **Narration — every turn.** Every subagent turn lands in the thread. (A
  server-picks-notable-only filter can come later; start dumb and complete.)
- **Chat is talk-only.** A user message can be answered from MEMORY + current
  progress state, but it does **not** directly steer progress — no writing
  MEMORY, reprioritizing, or spawning/killing subagents from chat yet. Chat
  observes and replies; steering comes later (likely mediated through MEMORY).
- **Progress is autonomous (default, not yet confirmed).** The server keeps
  progress moving on its own — spawn a bounded subagent run, narrate it, spawn
  the next — like #778's outer loop, but as a reaction inside the server rather
  than a standalone loop. Chat runs independently alongside it. Revisit if Jack
  wants progress gated on something.
