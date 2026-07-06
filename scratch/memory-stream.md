# memory-stream: full-fact, replayable memory stream

Linear: `memory-stream: full-fact, replayable memory stream`
(id `5d9d3dcc-80b2-439d-8771-d679e0191c30`). First slice of the Memory wave —
the foundation slices 2–4 stand on.

## Problem

A running mind can't read another mind's working memory; the only externalized,
inspectable form is `MEMORY.md`. Between the file and the live agents there's
supposed to be a *stream*: `lf memory add` publishes an immutable fact, running
agents subscribe and fold it. Today that stream is a lie in two ways:

1. **It carries a summary, not the fact.** `append_memory` broadcasts the
   one-line `summary`, never the full content (`runtime.rs:737`). A subscriber
   can't fold what it didn't receive — it would have to round-trip to
   reconstruct meaning. GOAL metric: *"The stream carries full facts, not
   summaries."*
2. **It's live-only — no replay.** The `memory` SSE frame fires on the live
   broadcast and is gone (`server.rs:591`, `:713`). A subscriber that connects
   one second late, or reconnects after a blip, silently loses every fact added
   while it was away. GOAL metric: *"No learning is lost between sessions … a
   new agent … replays the stream since; nothing an earlier agent knew silently
   disappears."*

Who benefits: every folding mind — the wave's resident and any worker running
`lf sub` — and every later slice, all of which assume a stream that actually
delivers facts.

## The demo

Two panes. Pane A: `lf sub`. Pane B: `lf memory add "workers report via lf
chat"`. Pane A prints the **full fact**, not a truncated summary. Now the part
that's new: kill pane A's `lf sub`, add two more facts in pane B, restart `lf
sub` — it **replays the two adds it missed**, in order, before going live.

The unit test performs the same thing without tmux: append N facts, subscribe
fresh, assert all N arrive with full content and in order.

## Approach

Give memory-adds their own journaled event, their own broadcast, and — the
crux — their own **replayable snapshot**, exactly the way turns already work.
Externalization (`lf memory update`) stays on its existing live-only `memory`
frame, untouched. Everything here is additive: the current `MEMORY.md` append
in `append_memory` stays (slice 2, "add becomes pure publish", removes it).

Five moving parts, all under the existing append lock so journal order,
snapshot order, and broadcast order agree:

1. **New journal event.** `EventKind::MemoryAdded { fact: String }` alongside
   `MemoryUpdated { summary }` (`journal.rs:260`). `append_memory` journals
   `MemoryAdded { fact }`; `update_memory` keeps journaling `MemoryUpdated`.
   The two events mean different things and must stay distinct in the record.

2. **A replay buffer that resets on externalization.** `Inner` gains
   `memory_adds: Vec<String>`, maintained incrementally under the lock like the
   turn cache: `append_memory` pushes the fact; **`update_memory` clears it.**
   The buffer is therefore *the adds since the last externalization* — which is
   exactly the model in GOAL.md ("`MEMORY.md` is a checkpoint of a mind's
   compiled state"). A fresh subscriber reads compiled state from `GET /memory`
   and replays only the facts added *since* that checkpoint — no double-count,
   and it pre-composes slice 2 for free. The journal fold gains the same
   accumulate-on-`MemoryAdded` / clear-on-`MemoryUpdated` logic so a server
   restart rebuilds the buffer correctly from disk (`fold_thread`,
   `journal.rs:905` currently ignores memory events).

3. **A dedicated broadcast.** `memory_add_tx: broadcast::Sender<String>` next to
   `memory_tx` (`runtime.rs:223`). `append_memory` sends the full fact. This is
   consistent with the codebase — turns, states, memory-update, and inbox each
   own a broadcast; memory-add earns its own rather than overloading `memory_tx`
   with a typed enum.

4. **Snapshot + live in one atomic subscribe.** `Subscription`
   (`runtime.rs:136`) gains `memory_adds: Vec<String>` (the snapshot) and
   `memory_add_rx` (live). `subscribe_with_snapshot` clones `inner.memory_adds`
   and subscribes `memory_add_tx` under the same lock as the turn snapshot
   (`runtime.rs:768`) — no gap, no overlap, no frame older than the snapshot.

5. **SSE wires it up, additively.** New event name `memory-add`, carrying the
   full fact as string data. In `events_handler` the primary `replay` stream
   (`server.rs:691`) chains the memory-add snapshot after the turn snapshot; a
   new `live_memory_adds` `filter_map` joins the live `select` beside
   `live_memory` (`server.rs:713`). The existing `memory` frame is unchanged, so
   every current consumer (Concerto, Swift) keeps working untouched.
   `sub.rs:174` renders `memory-add` as the full fact; `memory` stays the
   externalization marker.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does memory already ride the replayable snapshot, so this is a small tweak? | **No.** `subscribe_with_snapshot` snapshots turns + state only; memory is a bare live `memory_tx.subscribe()` (`runtime.rs:775`). The `replay` stream chains state + turns + inbox — no memory (`server.rs:691`). Replay is genuinely new plumbing, not a flag flip. | Design adds a real snapshot field (`Subscription.memory_adds`) backed by an `Inner` buffer, plus a replay chain — not "MemoryAdded rides the existing snapshot." |
| What does `append_memory` actually look like? | It's already `append_memory(&self, fact: &str, summary: &str)` and journals `MemoryUpdated{summary}` + broadcasts `summary` (`runtime.rs:726`). The original design's proposed `append_memory(&self, fact)` signature was stale. | Change the *body*: journal `MemoryAdded{fact}`, push to the buffer, broadcast `fact` on `memory_add_tx`. Keep the `summary` param (still used for the file bullet and `PostMemoryResponse`). |
| Can the fold reconstruct the buffer after a restart? | `fold_thread` walks events in order and already ignores `MemoryUpdated` (`journal.rs:905`). Accumulate-on-add / clear-on-update is a two-line addition to the existing match. | Restart rebuilds "adds since last externalization" deterministically from the journal — the buffer is not a volatile-only structure. |
| Will a new `memory-add` frame break existing SSE consumers? | The `memory` frame is a bare string on the wire, parsed as a string by `sub.rs`. Changing its shape would be a wire break; a *new* event name is purely additive. | New event `memory-add`; `memory` untouched. Honors CLAUDE.md "additive only" for this slice. |
| Is the SSE frame a wire DTO subject to the no-defaults rule? | It's an SSE `event: memory-add` + string `data`, not a serde struct with optional fields. No DTO mirror, no defaults concern. | Keep the fact as raw string data (required by construction — there is no absent case). Don't invent a JSON object DTO. |
| Does a lagged live subscriber lose adds? | The broadcast is lossy past `MEMORY_BROADCAST_CAPACITY` (`runtime.rs:57`), same as turns/state. But a reconnect gets a fresh snapshot. | A lagging subscriber resyncs by reconnecting (fresh `memory_adds` snapshot). Documented, not a blocker — matches how turns resync from `/conversation`. |
| Does the buffer grow unbounded? | Within one server life, yes — but so does the journal and turn cache. Externalization is *forced* at compaction and land (later slices), and server lives are bounded by land/restart. | Accepted bound for slice 1. Externalization (which clears the buffer) is the release valve; no eviction logic in this slice. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Overload `memory_tx` with a typed enum `MemoryFrame::{Added, Updated}` on one channel | One broadcast, one SSE frame with a `kind` discriminant | Turns the `memory` wire frame into a structured object — a break for existing consumers — and buries two different lifecycles (replayable fact vs. live-only marker) in one stream. A second broadcast is idiomatic here and additive. |
| Replay *all* adds since server boot (ignore externalization) | Simpler: buffer never clears | A fresh subscriber then double-counts every fact already compiled into `MEMORY.md`. Reset-on-externalization is barely more code, is more correct, and matches the GOAL's checkpoint model exactly. The fold's dedup can't fully paper over replaying already-compiled facts as "new." |
| Snapshot memory by re-folding the journal on each subscribe | No `Inner` buffer to maintain | The turn cache is maintained incrementally under the lock precisely to avoid per-subscribe re-folds; memory should match that shape, not diverge. The fold logic still exists — for restart reconstruction — but the hot path reads the cached buffer. |
| Include past `MemoryUpdated` summaries in the replay | Subscriber sees externalization history | Summaries are transient markers whose durable form is `MEMORY.md` itself (read via `GET /memory`). Replaying old summaries is noise, not learning. |

## Key decisions

- **The replay buffer is adds-since-last-externalization, not adds-since-boot.**
  This is the load-bearing choice. It makes a fresh subscriber's seed exactly
  `MEMORY.md` (compiled checkpoint) + the stream (uncompiled facts since), with
  no overlap, and it means slice 2 ("add becomes pure publish") drops the file
  write without disturbing replay semantics.
- **Memory-add gets its own broadcast and its own SSE event name.** Additive,
  idiomatic, and it keeps externalization's live-only `memory` frame byte-stable
  for existing clients.
- **`MEMORY.md` append stays this slice.** Removing it is slice 2. Keeping the
  diff additive keeps the demo honest and the change reviewable.
- **Ordering is guaranteed by the existing append lock**, not by new
  synchronization. Snapshot clone and `memory_add_tx.subscribe()` happen in the
  same `self.inner()` critical section as the turn snapshot.

## Scope

- **In scope:** `EventKind::MemoryAdded`; fold accumulate/clear logic; `Inner`
  buffer + push/clear in `append_memory`/`update_memory`; `memory_add_tx`;
  `Subscription.memory_adds` + `memory_add_rx`; `subscribe_with_snapshot`
  snapshot; `events_handler` replay chain + live stream; `memory-add` SSE frame
  and its `sub.rs` rendering; tests.
- **Out of scope:** removing the `MEMORY.md` file append (slice 2 —
  `adf893b4`); typed blocks in `MEMORY.md` (`2ae8a684`); forced externalization
  at land/compaction (`16869842`); any durability across land/branch/machine —
  that boundary is `MEMORY.md`'s, and the journal is per-machine and gitignored;
  eviction/bounding of the replay buffer.

## Done when

```bash
cargo test -p loopflow            # new MemoryAdded journal + replay tests pass
cargo clippy -- -D warnings && cargo fmt --check
```

New tests (drive the behavior without tmux):

1. **Full fact survives.** `append_memory(long_fact, short_summary)`; subscribe
   fresh; assert the `memory-add` frame data == `long_fact`, not `short_summary`.
2. **Late subscriber replays in order.** Append facts A, B, C; subscribe fresh;
   assert three `memory-add` frames, A→B→C.
3. **Externalization resets the buffer.** Append A, B; `update_memory(...)`;
   append C; subscribe fresh; assert exactly one `memory-add` frame (C), and
   that `GET /memory` still carries the compiled content.
4. **Restart reconstruction.** Fold a journal containing
   `MemoryAdded, MemoryAdded, MemoryUpdated, MemoryAdded`; assert the folded
   buffer is `[third add]` only.

Manual: the two-pane demo above — the reconnecting `lf sub` replays missed adds
in order.

## Wave alignment

Serves GOAL.md directly and advances two of its metrics verbatim:
*"No learning is lost between sessions — a new agent … replays the stream since"*
(replay) and *"The stream carries full facts, not summaries"* (full fact). Stays
inside the wave's stated boundary — replay is within a server's life; `MEMORY.md`
remains the only cross-boundary carrier (GOAL: *"the stream replays within a
server's life; MEMORY.md is the only thing that crosses land, branch, and
machine"*). No new risk introduced: the reset-on-externalization buffer is
strictly more aligned with the checkpoint model than a boot-scoped buffer would
be. Excludes the compaction hook — GOAL's hardest unknown — which this slice
doesn't touch.

## Measure

Not quantitative. The bar is behavioral: the full fact survives the round-trip,
and a subscriber reconnecting within a server's life loses no add — and, past an
externalization, replays only what isn't already compiled into `MEMORY.md`.
