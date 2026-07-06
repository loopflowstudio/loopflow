# memory-stream: full-fact, replayable memory stream

First slice of the Memory wave. Foundation the rest stands on.

## What to build

A memory add becomes a first-class streamed event carrying the **full fact**
(not a summary), journaled so a new or reconnecting subscriber **replays** every
add since it connected. Additive: `add`'s current file-append behavior stays;
this slice makes the *stream* real. (Slice 2 removes the file write.)

## The demo

Two panes. Pane A: `lf sub`. Pane B: `lf memory add "workers report via lf chat"`.
Pane A prints the full fact — not a truncated summary. Kill pane A's `lf sub`,
add two more facts, restart `lf sub`: it replays the adds it missed, in order.

## Current state (what changes)

- `EventKind::MemoryUpdated { summary }` (`journal.rs:260`) carries only a
  summary; the fold ignores it (`journal.rs:905`).
- `add` appends a bullet to `MEMORY.md`, journals a summary, broadcasts the
  summary string on `memory_tx` (`runtime.rs:726`).
- SSE `memory` event data = summary; **live-only, no replay** (`server.rs:591`,
  `:51-53`). `lf sub` renders `memory curated: <summary>` (`sub.rs:174`).

## Data structures

New journal event for an add — full content, distinct from externalization:

```rust
// journal.rs — alongside MemoryUpdated
EventKind::MemoryAdded { fact: String }   // an add: full fact, streamable, replayable
// MemoryUpdated { summary } stays: it marks an externalization (lf memory update)
```

Broadcast the fact, not a summary. `memory_tx` already exists
(`runtime.rs:223`); the payload it carries becomes the full fact for adds.

## Key functions

```rust
// runtime.rs
fn append_memory(&self, fact: &str) -> …   // journal MemoryAdded{fact}; broadcast fact.
                                            // (keeps the MEMORY.md append for now — slice 2 drops it)

// server.rs — /events
// include MemoryAdded in the replayable snapshot+tail, like other events already are
// via subscribe_with_snapshot (runtime.rs:775). The memory frame stops being live-only.

// sub.rs
// render MemoryAdded as the full fact; render MemoryUpdated as the externalization marker
```

## Constraints

- **Replay scope is within a server's life.** The journal is per-machine and
  gitignored; replay never crosses land/branch/machine. That boundary is
  `MEMORY.md`'s job (later slices). Don't try to make the stream durable across
  branches.
- **Additive only.** Do not change what `add` writes to `MEMORY.md` in this
  slice — that's slice 2. Keeping it additive keeps the demo honest and the diff
  small.
- **DTO discipline.** The SSE `memory` frame and any `/memory` response are wire
  types mirrored to clients — no serde defaults; full fact is a required field,
  not an `Option` masquerading as "empty is fine" (see CLAUDE.md DTOs).
- **Ordering.** Adds are ordered; replay must preserve journal order under the
  same lock that append already takes.

## Done when

```bash
cargo test -p loopflow            # new tests for MemoryAdded journal + replay pass
cargo clippy -- -D warnings && cargo fmt --check
```

Manual: run the two-pane demo above; the reconnecting `lf sub` replays missed
adds in order. A unit test drives it without tmux: append N facts, subscribe
fresh, assert all N arrive with full content.

## Measure

Not quantitative. The bar is behavioral: full fact survives the round-trip, and
a late subscriber loses nothing within the server's life.
