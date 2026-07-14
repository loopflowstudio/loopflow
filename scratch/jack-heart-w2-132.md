# W2-132 — Wave Chat token streaming: measured baseline

Baseline pinned to base commit `4fbc980f`. No rebase, no optimization in the
measurement. W2-129 (transcript coalescing) is isolated in another worktree and
does not touch these numbers; any coalescing change is a separate post-change
measurement.

Measured on the living workspace — the real `product` wave journal
(`~/src/loopflow/.lf/journal/waves/product/journal.jsonl`), not a demo state.

## Transcript size (the accumulated state everything is measured against)

| | |
|---|---|
| journal events | 9,094 |
| journal bytes | 4.7 MB |
| thread turns | 90 |
| `/events` connect replay | **3.17 MB** |
| representative turn (`turn-626`) | 657 deltas, 3,149 chars of prose, 49 tool items |
| that turn's frame at close | **106 KB** |

## The two headline numbers

**Time to first visible token: ~22.6 s.** On connect, `/events` replays every
turn in the thread (`server.rs:653`) — 3.17 MB — and the Mac drains it at
0.14 MB/s (below). Nothing renders until that finishes.

**Sustained visible-token rate: ~1.4 deltas/s, decaying to ~1.3.** One 106 KB
frame takes 734 ms to read. This is the reported "couple of words per second",
and it is now explained exactly: the observed 1.3 deltas/s in the real journal
matches the predicted drain rate.

## Where the time goes, per delta

Stages, measured independently, for one provider text delta on the accumulated
transcript:

| stage | cost per delta | verdict |
|---|---|---|
| provider → listener (in-burst) | 8 ms (125 deltas/s) | healthy — the provider is not slow |
| listener fold + journal + broadcast | **5 µs** | noise |
| Mac: SSE read of the frame | **734 ms** | **dominant — 99.9%** |
| Mac: JSON parse of the frame | 0.44 ms | noise |
| Mac: TextKit relayout of the turn | 0.39 ms | noise |

The listener sustains 171,000 deltas/s. The Mac drains 1.4/s. Everything
between them is rounding error.

## The dominant stage

Two independent facts multiply.

**1. The listener re-broadcasts the entire open turn on every delta.**
`append_turn_item_locked` (`wave/runtime.rs:1202`) clones the open `ChatTurn`,
re-serializes it whole, and sends it. The turn carries not just the prose but
every accumulated `item` — including full tool outputs. So the frame grows from
a few hundred bytes to 106 KB across a turn, and one turn puts **68.6 MB** on
the wire to deliver 3,149 characters of prose. That is 21,800× amplification.

The code already predicted this. `runtime.rs:1016` reasons that no throttle is
needed because "deltas are item-granular from the vendor stream (one per
completed item, **not per token**)" — and `TurnFrame` (`runtime.rs:104`) notes
"the delta-granular wire — sending increments instead of whole turns — stays
future work." The first claim is false today: `flowloop/wave.rs:864` forwards
every provider `TextDelta` straight through, one per token. The design's own
stated condition for revisiting this has been met.

**2. The Mac reads SSE one byte at a time, on the main actor.**
`WaveChatClient.stream` did `for try await byte in bytes` over
`URLSession.AsyncBytes`, feeding `SSEFrameParser.consume` one `UInt8` per
`await`. `WaveChatConnection` is `@MainActor`, so **every byte cost a
main-actor hop**. Measured on a real connection: **0.14 MB/s**.

The isolation is the whole story, and it is easy to measure wrong: the same
byte loop off the main actor runs at ~20 MB/s, and fed from a `URLProtocol`
stub instead of a socket it is fast too. A regression test that misses either
detail passes on the broken path — both were tried, both passed, both were
thrown away.

Neither fact alone would be fatal. Together: a 106 KB frame per token, read at
0.14 MB/s, on the main actor.

## Proposed budgets

Named before optimizing, so the gate has something to fail against:

- **Time to first visible token ≤ 500 ms** on the worst transcript we have.
- **Sustained visible-token rate ≥ the provider's rate** (~125 deltas/s in
  burst). The reader must never be the bottleneck; if it cannot keep up, the
  broadcast channel drops frames (`server.rs:711` swallows `Lagged`) and the
  transcript silently skips.
- **No per-token stage may scale with turn length or transcript length.** Both
  violations above are of this rule; it is the one that generalizes.

## The fix (landed here)

**Read SSE in `Data` chunks, keeping the byte-level parse.** `SSEChunkStream`
feeds `SSEFrameParser` whole chunks from a `URLSessionDataDelegate`; the byte
loop inside a chunk is synchronous, so it costs one main-actor hop per network
read instead of one per byte.

The obvious move — `bytes.lines` — is wrong, and the code already said so:
`AsyncLineSequence` drops empty lines, and the empty line is exactly what
terminates an SSE frame. Chunking keeps the parser that handles this correctly
and changes only how it is fed.

Post-change, against the pinned baseline:

| | before | after |
|---|---|---|
| SSE read throughput | 0.14 MB/s | >200 MB/s |
| 106 KB frame | 734 ms | ~0.5 ms |
| 3.17 MB connect replay (TTFT) | 22.6 s | **0.058 s** |
| sustained visible-token rate | 1.4 deltas/s | bounded by the provider, not the reader |

Both budgets are met, and the reader is no longer the bottleneck at any point
in the path.

## Still open: the wire shape

**The listener should stop re-broadcasting whole turns per token** — the
delta-granular wire `runtime.rs:104` already names as future work. 68.6 MB per
turn is wrong regardless of how fast the reader is, and it is what makes the
`Lagged` frame-drop path (`server.rs:711`, which silently swallows lagged
frames) reachable at all. It is a wire-shape change across Rust, Swift, and the
DTO mirrors, and it is not needed to hold the budgets above — so it is filed
separately rather than riding this change.

## Harnesses (committed, repeatable)

- `swift/LoopflowTests/WaveChatStreamTests.swift` — **the gate**. Streams a
  3.2 MB replay over a real loopback socket and holds a 5 s budget. Verified to
  bite: swapped back to the byte-at-a-time transport it takes **21.2 s** and
  fails.
- `rust/loopflow/benches/wave_stream.rs` — listener stage. `cargo bench --bench
  wave_stream`; `LF_BENCH_JOURNAL` points it at a real journal.
- `swift/Benchmarks/wave_chat_render.swift` — Mac parse + relayout stage,
  replayed delta-by-delta against a recorded real turn.

Two things the gate must not do, both learned by building them and watching
them pass on the broken path: it must use a **real socket** (over a
`URLProtocol` stub, `AsyncBytes` is fast) and it must run on the **main actor**
(off it, the byte loop is ~140x faster). Either mistake yields a green test
over a 160x regression.
