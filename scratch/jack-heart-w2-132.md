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

**2. The Mac reads SSE one byte at a time.** `WaveChatClient.stream`
(`WaveChatClient.swift:287`) does `for try await byte in bytes` over
`URLSession.AsyncBytes`, feeding `SSEFrameParser.consume` a single `UInt8` per
async suspension. Measured throughput: **0.14 MB/s**, compiled `-O` (the
suspension overhead dominates; optimization level does not move it).

Neither alone would be fatal. Together: a 106 KB frame per token, read at
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

## The fix, and why it is two changes

Measured, not assumed:

1. **Read SSE by line, not by byte.** An SSE frame is one `data:` line, so
   `bytes.lines` is a drop-in for the byte loop. Measured: **133 MB/s**, a 950×
   improvement. Frame read 734 ms → 0.8 ms; connect replay 22.6 s → 0.02 s.
   This alone takes the reader off the critical path and is a small, contained
   change to `WaveChatClient`.

2. **Stop re-broadcasting whole turns per token** — the delta-granular wire the
   runtime comment already names as future work. Worth doing on its own merits
   (68.6 MB per turn is absurd regardless of how fast the reader is, and it is
   what makes `Lagged` frame-drops reachable), but it is a wire-shape change
   touching Rust, Swift, and the DTO mirrors. It is not needed to hit the
   budgets above, so it should not ride the same change.

Order matters: (1) is the one that moves the measured numbers, so it lands
first with a post-change measurement against this baseline. (2) is filed
separately.

## Harnesses (committed, repeatable)

- `rust/loopflow/benches/wave_stream.rs` — listener stage. `cargo bench --bench
  wave_stream`; point `LF_BENCH_JOURNAL` at a real journal to measure the
  accumulated transcript rather than an empty one.
- `swift/Benchmarks/wave_chat_render.swift` — Mac parse + relayout stage,
  replayed delta-by-delta against a recorded real turn.

The reader-throughput measurement (0.14 vs 133 MB/s) is the number the
regression gate should hold; wiring it into CI comes with the fix, since a gate
on a known-broken path just fails on day one.
