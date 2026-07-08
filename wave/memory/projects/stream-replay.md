# Stream and replay

The delta stream works end to end: publish full facts, seed from the compiled
checkpoint, replay once, then live.

## KRs

- Live demo: a fresh subscriber seeds from `MEMORY.md`, replays the delta
  once, then receives new facts live.
- Facts in the stream are complete enough for subscribers to fold without
  reconstructing from another source.
