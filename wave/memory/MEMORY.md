# Memory wave — compiled memory

Seeded from the design exploration on branch `memory-stream`. Dogfoods the block
structure the model proposes.

## Decisions

- **The fold lives in the mind, not in code.** Agents subscribe to the memory
  stream and consolidate into their own opaque working memory. There is no
  external consolidator process — the unit of work is a mind, and the mind holds
  the memory. This is why "you already have the thing that runs the fold."
- **`MEMORY.md` is a checkpoint of a mind's compiled state; the stream is the
  delta since.** A new agent = load `MEMORY.md` + subscribe to the stream (or
  replay it) → re-fold in its own context.
- **`add` is a pure publish** (proposed, being built): journal the full fact +
  broadcast. It stops writing `MEMORY.md`, so the file stays *compiled*, not an
  accreting pile of raw bullets. Today's code appends a bullet to the file — that
  changes.
- **Only the wave's mind externalizes `MEMORY.md`.** Workers only `add`. Work
  lines have no memory (targeting already resolves to the family head), so there's
  no last-writer-wins clobber. Matches the one-orchestrating-mind wave design.
- **Externalization is forced at context-compaction and at land** — the two
  moments an in-head fold would otherwise be lost.
- **Letta: learn-from only.** Reimplement blocks + fold. No backend, no server
  dependency, no vector store. Letta's "sleep-time agent" is, in loopflow terms,
  just a dispatched TUI session running the fold — loopflow expresses it more
  natively than importing it would.
- **Bounded, not unbounded.** Memory is context-sized; `MEMORY.md` is the whole
  compiled form. No archive, no retrieval.
- **Durability scopes:** replayable stream *within a server's life* (journal
  snapshot+tail); `MEMORY.md` is the only thing that crosses land/branch/machine.

## Constraints

- **Only `MEMORY.md` crosses the branch boundary.** The journal is per-machine
  and gitignored; its `MemoryUpdated` record carries only a summary and the fold
  ignores it. Agents seed from starting-branch state, so whatever must survive a
  land has to be *in the committed file*.
- **The server holds the pen.** Only the live wave server writes `MEMORY.md`,
  under a lock, journaled + broadcast (`runtime.rs:711,726`). Offline waves edit
  the file directly. This is what makes "discourage manual editing" enforceable.
- **We cannot read a running agent's internal memory representation.** The only
  way to get "whole compiled memory" out is a mind externalizing via `update`.
- **Compaction is owned by the vendor CLI.** Externalize-at-compaction assumes
  loopflow can act *before* Claude Code compacts. Unproven — the least-certain
  mechanism in the design. Fallback: land-externalization + periodic updates.

## Glossary

- **add** — publish an immutable fact to the append stream. `lf memory add`.
- **stream** — the ordered, broadcast log of adds. Subscribed via `lf sub`.
- **fold** — a mind consolidating incoming facts into its working memory.
- **externalize** — a mind writing its compiled state to `MEMORY.md` via
  `lf memory update`. The only checkpoint operation.
- **block** — a typed, budgeted region of the compiled `MEMORY.md`
  (decisions / constraints / roster / glossary).
- **checkpoint** — a committed `MEMORY.md`; the seed the next agent inherits.

## Code map (current state)

Memory is Rust-only under `rust/loopflow/src/`. `Memory { path }`
(`wave/memory.rs:19`); server writer (`runtime.rs:711,726`); routes
(`server.rs:343` `/memory`); CLI (`lf/commands/memory.rs`); SSE `memory` event is
summary-only, live-only, no replay (`server.rs:591`); injection via
`wave_memory_section` → `<lf:wave-memory>` (`engine/flow.rs:281`). No
PR-serialization logic; no blocks; no fold — all greenfield.
