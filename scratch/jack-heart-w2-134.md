# W2-134 — Delta-granular live-turn wire

Stop re-broadcasting the whole open turn per token. Clarify-phase design; ready
to build. (Directive v1 not yet acknowledged — acknowledge on resume.)

## The bug, precisely located

Only ONE broadcast is the flood. `WaveRuntime` fans open-turn growth to SSE
subscribers via `turn_tx: broadcast::Sender<Arc<TurnFrame>>`. `TurnFrame::share`
serializes the ENTIRE `ChatTurn` (prose + every accumulated item) once per send
(`rust/loopflow/src/wave/runtime.rs:104-118`). Four send sites:

- `resident_turn_opened` (runtime.rs:1146) — whole, tiny empty turn, once/turn. **keep whole**
- `append_turn_item_locked` (runtime.rs:1192-1201) — whole, **PER TEXT DELTA** → the flood. **← the fix**
- `update_body_session` (runtime.rs:548) — whole, rare (once/turn). **keep whole**
- `push_turn`/`commit` finalize (runtime.rs:849) — whole, once/turn. **keep whole**

`resident_turn_text` (runtime.rs:1155) wraps each token into
`ConversationItem::Message{ id: "text-N", phase: Some("stream") }` and calls
`append_turn_item_locked` → `ChatTurn::absorb_item` (concatenates stream
Messages into `.text`, appends everything else to `.items`) → `TurnFrame::share`
of the whole turn. So the wire cost is O(prose²): 3,149 chars of prose → 68.6 MB,
frames to 106 KB, 21,800x amplification.

The three surviving whole sends are all O(1) per turn — leave them. They double
as free authoritative re-baselines for the client mid-turn.

## Design: snapshot stays whole, live growth becomes increments

**Insight:** the increment the client needs is exactly the `ConversationItem`
already being absorbed. The server grows its open turn through `absorb_item`;
if the client applies the SAME `absorb_item` to the SAME items in order, its
reconstruction is byte-identical to the server's open turn. The finalized whole
turn (sent at commit) then re-baselines and papers over any drift at turn close.

### Wire (SSE `/events`)

- `turn` event (unchanged): whole `ChatTurn`. Still used for replay snapshot on
  connect, user turns, child-activity turns, the open skeleton, body-session
  update, and the finalized turn. Client rule: **replace-by-id** (Swift `upsert`,
  Rust `thread.rs` already diff by id).
- NEW `turn-delta` event: one increment to an open assistant turn. Payload DTO
  `{ turn_id: String, item: ConversationItem }`. Client rule: find turn by id,
  `absorb_item(item)`. If id unknown → missed the open → treat as desync (resync).
- NEW `resync` event (no data): emitted when the turn broadcast reports `Lagged`.
  "Your open-turn reconstruction may have a gap; discard it and reconnect."

### Rust runtime

- Change the broadcast payload from `Arc<TurnFrame>` to an enum carrying either a
  whole frame or a delta frame, each pre-serialized ONCE at the send site (keep
  the Arc-share: N subscribers, one serialization). Sketch:
  `enum TurnBroadcast { Whole(Arc<TurnFrame>), Delta(Arc<TurnDeltaFrame>) }`
  where `TurnDeltaFrame { turn_id, item, json }`.
- `append_turn_item_locked`: journal the `TurnItem` as today, but broadcast a
  `Delta{turn_id, item}` (NOT `open.clone()`). Still absorb into the in-memory
  open turn so `/conversation` and reconnect snapshots stay whole.
- Other three send sites: unchanged (`Whole`).
- Delete the now-false comments: runtime.rs:104-106 ("delta-granular wire stays
  future work"), runtime.rs:1006-1011 ("deltas are item-granular... a throttle
  earns its place with the part-grained wire"). The condition they named is met.

### Lag = explicit resync, not silent drop (server.rs:711-727)

`live_stream` currently does `res.ok()` — silently dropping BOTH `Lagged` and
`Closed`. For a whole-turn wire a dropped frame is harmless (next whole frame
replaces state); for a delta wire a dropped delta permanently corrupts the
reconstruction. Fix: a dedicated turn substream that maps `Ok → turn|turn-delta`
event, `Err(Lagged) → emit resync event then END the whole /events response`,
`Err(Closed) → end`. Ending the response leans on both clients' existing
reconnect-and-reset logic; the `resync` event makes it explicit (not silent).
Race-free because reconnect uses `subscribe_with_snapshot` (atomic snapshot +
receiver, broadcasts under the append lock) — a side `/conversation` refetch
would race the stream cursor, a full `/events` reconnect does not.

### Swift (`swift/Loopflow/Services/WaveChatClient.swift`, `Models/`)

- `handle(event:data:)`: add `"turn-delta"` → decode `TurnDeltaFrame`, apply to
  `turns` by id via a new `ChatTurn.absorbing(_:) -> ChatTurn` (mirror of Rust
  `absorb_item`; `ChatTurn` is currently immutable `let` fields — add a functional
  copy that returns a new turn with the item absorbed). Unknown id → resync.
- `"resync"` → break the read loop so `run()` reconnects; `stream()` already
  resets `turns=[]` on each connect, so reconnect fully heals.
- `ConversationItem` mirror (ConversationTypes.swift:197-305) already complete;
  reuse it in the delta payload.

### Rust CLI `lf chat --follow` (thread.rs)

- Renderer already tracks per-turn progress and diffs prose by `text_chars`. Add
  `"turn-delta"`: maintain a reconstructed `ChatTurn` per id, `absorb_item`, then
  reuse `turn_lines`. On `"resync"`, return from `stream_events` so the `follow`
  loop reconnects (its state resets).

### DTO fixtures (CLAUDE.md rule)

Add `tests/fixtures/dto/turn_delta.json` for `TurnDeltaFrame` and wire it into
the Rust + Swift + Python fixture tests. `resync` carries no data (no DTO).

## Tests (user-facing, pin the two behaviors this task exists for)

1. **Reconstruction identity** — feed a turn's `TurnOpened` + N `TurnText`/`TurnItem`
   deltas; assert the client's `absorb`-reconstructed open turn == the server's
   finalized whole turn, byte for byte. Rust runtime test + Swift `WaveChatConnection`
   test.
2. **Amplification** — assert per-delta wire bytes are O(fragment), not O(turn):
   a growing turn's delta frames stay bounded (~tens of bytes), total wire is
   O(prose) not O(prose²). Contrast against the baseline in PR #877
   (`scratch/jack-heart-w2-132.md`).
3. **Lag → resync, not silent gap** — force the turn broadcast to lag; assert an
   explicit `resync` is emitted and the stream ends (client reconnects), rather
   than a silently-skipped delta. Directly covers server.rs:711.

## Scope / invariants

- One PR, one worktree. Wire change across Rust + Swift + DTO mirrors; Python
  fixture test only (no Python consumer of the live turn stream).
- The resident↔listener wire (`ResidentDelta`, wire.rs) is Rust↔Rust and does NOT
  change — the flood is purely the listener→subscriber `/events` boundary.
- Preserve full durable replay: journal fold and `/conversation` are untouched;
  only the LIVE broadcast granularity changes.
- Keep one implementation of turn growth: client and server both grow through
  `absorb_item`. Do not fork the rule.

## Files

- rust/loopflow/src/wave/runtime.rs — TurnFrame/broadcast enum, append_turn_item_locked, dead comments
- rust/loopflow/src/wave/server.rs — turn-delta + resync events, lag substream (511-727)
- rust/loopflow/src/lf/commands/thread.rs — Renderer turn-delta + resync
- swift/Loopflow/Services/WaveChatClient.swift — handle(), reconnect on resync
- swift/Loopflow/Models/ChatTurn.swift — absorbing(_:) helper
- rust/loopflow/tests/fixtures/dto/turn_delta.json (+ Rust/Swift/Python fixture tests)
