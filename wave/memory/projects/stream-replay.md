# Stream and replay

The delta stream works end to end. Audited 2026-07-08: the machinery is
intact and tested (runtime.rs:1509,1529) — and the wire shape is
seed-out-of-band: a subscriber fetches MEMORY.md via `GET /memory`, then
SSE replays the `memory-add` delta since the last compile, then live. There
is no MEMORY.md seed frame on the wire, by design.

## KRs

- Live demo: fresh subscriber does `GET /memory` seed + delta replay once +
  live facts, across a wave restart.
- Facts in the stream stay complete enough to fold without another source.
- The `memory` curation-summary event is live-only (no replay) — confirm
  that's sufficient for late subscribers or add replay deliberately.
