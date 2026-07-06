## Try it!

```bash
cargo test -p loopflow wave::runtime::tests::subscription_replays_full_memory_facts_in_order
cargo test -p loopflow wave::tests::memory_routes_read_and_write_through_the_server
```

Run a live wave and subscribe in another pane:

```bash
lf wave memory --no-mind
lf sub memory
lf memory add "workers report durable facts" --wave memory
lf memory update --wave memory < wave/memory/MEMORY.md
```

`lf sub` prints the add as `memory added: ...`. After `update`, a reconnect seeds from `MEMORY.md` and does not replay older add facts.

Validation run here:

```bash
cargo fmt --check
cargo clippy -- -D warnings
uv run python scripts/test.py
uv run python scripts/test.py --all
```

Full runner result: Python, Rust, website, Swift, and e2e passed. Concerto UI failed before XCTest bootstrap with `ConcertoUITests-Runner` killed before connecting; result bundle is at `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-dptljcblnlxvwdbazshrscvzqnra/Logs/Test/Test-Concerto-2026.07.06_14-37-19--0700.xcresult`.

## Intent

Make wave memory additions replayable as full facts without turning `MEMORY.md` into an append log. A new subscriber should reconstruct memory as compiled checkpoint plus the add delta since that checkpoint.

## Assumptions

`MEMORY.md` is the cross-branch and cross-machine checkpoint. The journal-backed add stream is local runtime state. The wave listener remains the only live writer for memory routes.

## Key Decisions

`lf memory update` stays as the externalization command. It writes the compiled checkpoint, journals `MemoryUpdated`, emits the live `memory` summary event, and clears replayed adds.

`lf memory add` publishes `MemoryAdded { fact }`, broadcasts `memory-add`, and does not mutate `MEMORY.md`. The server still returns `{summary}` so the existing CLI response shape remains simple.

Swift `memory` summary handling stays in place because update events still exist. `memory-add` is additive.

## Not Included

Typed memory blocks, forced land/compaction externalization, and cross-machine stream replay are left for later slices.
