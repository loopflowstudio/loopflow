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
- **`add` publishes a full fact to a replayable stream** (shipped, slice 1):
  journals `MemoryAdded { fact }`, pushes to a replay buffer, broadcasts the
  full fact on its own channel. A fresh subscriber replays the adds since the
  last externalization, then goes live. `add` *still* appends a bullet to
  `MEMORY.md` today — making it a pure publish (dropping the file write so the
  file stays *compiled*, not an accreting pile of raw bullets) is slice 2
  (`adf893b4`).
- **The replay buffer is adds-since-last-externalization, not adds-since-boot.**
  Load-bearing: it makes a fresh subscriber's seed exactly `MEMORY.md` (compiled
  checkpoint) + the stream (uncompiled delta), no overlap, no double-count.
  `append_memory` pushes; `update_memory` clears. The journal fold applies the
  same accumulate-on-`MemoryAdded` / clear-on-`MemoryUpdated` logic, so a server
  restart rebuilds the buffer deterministically from disk.
- **memory-add earns its own broadcast and SSE event name.** `memory-add`
  (full facts, replay-then-live) sits beside `memory` (curation summaries,
  live-only). The `memory` frame stays byte-stable for existing consumers —
  additive, not a wire break.
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

- **Only `MEMORY.md` crosses the branch boundary.** The journal (and the
  `MemoryAdded` replay buffer it rebuilds) is per-machine and gitignored, so the
  stream replays only *within a server's life*. Agents seed from starting-branch
  state, so whatever must survive a land has to be *in the committed file*.
- **The server holds the pen.** Only the live wave server writes `MEMORY.md`,
  under a lock, journaled + broadcast. Offline waves edit the file directly.
  This is what makes "discourage manual editing" enforceable.
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
(`wave/memory.rs`); routes (`server.rs` `/memory`, `/events`); CLI
(`lf/commands/memory.rs`); injection via `wave_memory_section` →
`<lf:wave-memory>` (`engine/flow.rs`).

Shipped (slice 1, `memory-stream`):
- `EventKind::MemoryAdded { fact }` beside `MemoryUpdated { summary }`
  (`journal.rs`); `fold_thread` materializes `ThreadFold.memory_adds`
  (accumulate on add, clear on update).
- `WaveRuntime` seeds `Inner.memory_adds` from the fold, maintains it under the
  append lock, fans facts out on `memory_add_tx`. `append_memory(&self, fact)`
  (the summary arg was dropped — the file bullet *is* the fact); `update_memory`
  clears the buffer.
- `Subscription.memory_adds` (snapshot) + `memory_add_rx` (live), cloned+
  subscribed atomically in `subscribe_with_snapshot`.
- SSE `memory-add` event: replay chain then live, beside the live-only `memory`
  frame. `lf sub` renders it as the full fact ("memory added: …").

Still greenfield: pure-publish `add` (file write removal, slice 2); typed
blocks (slice 3); forced externalization at land/compaction (slice 4). No
cross-machine/branch replay — that boundary is `MEMORY.md`'s; the journal is
per-machine and gitignored.
