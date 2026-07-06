# memory-stream: full-fact, replayable memory stream

Slice 1 of the Memory wave — shipped in PR #823. Linear item
`5d9d3dcc-80b2-439d-8771-d679e0191c30` (still `open`; see `questions.md` — the
`lf op pm` close mutation is broken). Design detail now lives in the code and in
`wave/memory/MEMORY.md`; this file keeps only the try-it recipe and the
done-when checks.

## What shipped

`lf memory add` publishes the **full fact** to a replayable stream: journals
`MemoryAdded { fact }`, buffers it, broadcasts on its own `memory-add` SSE
event. A fresh `/events` subscriber replays the facts added since the last
externalization, then continues live. `lf memory update` still fires the
live-only `memory` summary frame and clears the replay buffer, so `MEMORY.md`
stays the compiled checkpoint and the stream is the delta after it. The
`MEMORY.md` bullet append still happens (removing it is slice 2, `adf893b4`).

## The demo

Two panes. Pane A: `lf sub`. Pane B: `lf memory add "workers report via lf
chat"`. Pane A prints the **full fact**, not a truncated summary. Then: kill
pane A's `lf sub`, add two more facts in pane B, restart `lf sub` — it
**replays the two adds it missed**, in order, before going live. Past an
`lf memory update`, replay carries only the adds since that externalization.

## Done when

```bash
cargo test -p loopflow            # MemoryAdded journal + replay tests
cargo clippy -- -D warnings && cargo fmt --check
```

Behavioral tests (drive it without tmux), all present and passing:

1. **Full fact survives** — `memory-add` frame data is the long fact, not the summary.
2. **Late subscriber replays in order** — append A,B,C; fresh subscribe → A→B→C.
3. **Externalization resets the buffer** — append A,B; `update`; append C; fresh
   subscribe → exactly C; `GET /memory` still carries the compiled content.
4. **Restart reconstruction** — fold `Added,Added,Updated,Added` → buffer is `[third]`.
