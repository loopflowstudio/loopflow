# memory-stream gate review

## What was implemented

Adds a replayable memory-add stream for waves. `lf memory add` now journals
`MemoryAdded { fact }`, stores the full fact in an in-memory replay buffer,
and emits a new `memory-add` SSE frame. Fresh `/events` subscribers replay the
facts added since the last `MemoryUpdated`, then continue on the live stream.

`lf memory update` keeps the existing `memory` summary event and clears the
add replay buffer, so `MEMORY.md` remains the compiled checkpoint and the add
stream remains the delta after it.

## Key choices

- Keep `memory` byte-stable and add a new `memory-add` event instead of
  changing the existing frame shape.
- Store adds-since-last-externalization, not adds-since-boot, to avoid
  replaying facts already compiled into `MEMORY.md`.
- Rebuild the replay buffer from the journal fold on restart so reconnects
  after a server restart see the same add delta.
- Leave the current `MEMORY.md` append behavior in place for this slice; later
  slices can make add pure publish without changing the stream contract.

## How it fits together

The journal fold materializes `ThreadFold.memory_adds`. `WaveRuntime` seeds
`Inner.memory_adds` from that fold, updates it under the same append lock as
memory writes, and snapshots it in `subscribe_with_snapshot`. The SSE handler
chains the snapshot into replay before live broadcasts, mirroring the existing
turn replay shape.

`lf sub` renders `memory-add` frames as full facts. `memory` still means
curation/externalization summary.

## Risks and bottlenecks

- A very long period without externalization can grow the replay buffer. That
  matches the current slice's design boundary; externalization is the release
  valve.
- Live broadcast lag can still drop frames past channel capacity. Reconnect
  repairs this because the add snapshot is replayable.
- Existing consumers that ignore unknown SSE event names keep working. Consumers
  that assumed the complete event vocabulary may need to account for
  `memory-add`.

## What is not included

- Removing raw add bullets from `MEMORY.md`.
- Typed memory blocks.
- Forced externalization at land or context compaction.
- Cross-machine or cross-branch replay beyond committed `MEMORY.md`.

## Validation

Changed-aware plan:

```bash
uv run python scripts/test.py --list
```

It selected Rust only and skipped Python, website, Swift, e2e, and Concerto
because no matching paths changed.

Executed checks:

```bash
cargo fmt --check
cargo test -p loopflow
cargo clippy -- -D warnings
cargo test --all
```

All passed.

Review note: the gate pass found one stale wire-contract doc entry in
`rust/loopflow/src/wave/README.md`; it is fixed in this branch.
