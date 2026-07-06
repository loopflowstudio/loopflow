## Try it!

```bash
cargo test -p loopflow
cargo clippy -- -D warnings
cargo fmt --check
```

For the behavior directly: start a wave subscription, run `lf memory add "workers report via lf chat with full detail"`, and watch `lf sub` print the full fact through `memory-add`. Restart `lf sub` after adding facts and it replays the missed adds since the last `lf memory update`.

## Intent

Make wave memory adds replayable and fact-preserving. A subscriber should not lose learnings added while it was disconnected, and it should receive the full fact rather than a shortened summary.

## Assumptions

`MEMORY.md` is the compiled checkpoint. The replay buffer is only the delta after the last externalization, and `MEMORY.md` remains the only cross-branch, cross-machine memory carrier.

## Key decisions

- Add `MemoryAdded { fact }` beside a payload-free memory checkpoint event.
- Add a new `memory-add` SSE event so the existing `memory` summary event stays stable.
- Clear replayed adds on `lf memory update` to avoid double-counting facts already compiled into `MEMORY.md`.
- Rebuild the add replay buffer from the journal fold on restart.

## Not included

This does not remove raw add bullets from `MEMORY.md`, add typed memory blocks, or force externalization at land/compaction. Those are later memory-wave slices.

## Validation

Pending this gate run.
