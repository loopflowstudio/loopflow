---
priority: high
---

# lf loop — the chat loop + progress→chat streaming

**Finish line:** A separate chat loop lets a human message a running wave and get
answered *without waiting on the progress pass*; the progress loop streams live
updates into chat; steering flows back into the progress loop.

## Context

Progress loop and chat loop are concurrent and separate (design:
`scratch/jack-heart.lf-loop.md`). Chat is the lightest pass — it skips heavy
orientation, answers from MEMORY + streamed progress, and **dispatches solution
threads** rather than holding a long interactive session that itself gets stuck.

The dormant chat scaffolding the codebase already carries is exactly this
channel: `ChatMessage` + `ChatMemoryBlock` types, store methods, migrations
007/008, DTOs — all unwired (zero HTTP routes, constructors never called). Build
on it, don't reinvent it.

## What to shape

- **Wire the mailbox in lfd:** light up the chat DTOs + HTTP routes + WS-inbound
  routing so a client can post chat/steering to a running loop.
- **Progress → chat streaming:** the progress loop emits `ChatMemoryBlock`
  updates as it works; the chat loop (and any WS client) read them live.
- **Chat loop:** on a message, read MEMORY + streamed progress, answer, and
  dispatch a solution thread (an `lf goal -b` variant) if the ask needs work.
  Never a long-held session.
- **Steering back-edge:** a steering message lands in the mailbox → the next
  progress pass picks it up and reprioritizes.

## Done when

- A human posts a message to a running `lf loop` wave over HTTP/WS, gets a reply
  sourced from live progress while the progress loop keeps running, and a
  steering message visibly reprioritizes the next progress pass.
