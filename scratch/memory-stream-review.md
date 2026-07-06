# memory-stream Review

## What Was Implemented

Added a replayable memory-add stream for waves. `lf memory add "<fact>"` now publishes a full fact as `MemoryAdded { fact }`, stores it in the runtime replay buffer, and emits `memory-add` SSE frames to subscribers. `lf sub` renders those frames as full facts.

Kept `lf memory update` as the compiled checkpoint path. Updates replace `wave/<name>/MEMORY.md`, journal `MemoryUpdated { summary }`, emit the live-only `memory` curation event, and clear the replay buffer so a fresh subscriber seeds from `MEMORY.md` plus only the add delta since that checkpoint.

## Key Choices

- Preserve `lf memory update` instead of removing it. The loopflow operating prompt, docs, and memory model all rely on an explicit externalization command.
- Make `add` a publish operation, not a file append. Raw facts no longer accrete into `MEMORY.md`; the file remains the compiled checkpoint.
- Keep both SSE events: `memory` for curation summaries and `memory-add` for replayable facts. Existing Swift `memorySummary` handling stays intact.
- Rebuild replay state from the journal. `MemoryAdded` accumulates, `MemoryUpdated` clears.

## How It Fits Together

The listener remains the only pen. `/memory` handles both update and add under the runtime lock; the journal fold reconstructs the thread plus memory-add delta on restart. Subscribers receive the current thread, then replayed add facts, then live turn/state/memory events.

## Risks And Bottlenecks

- The replay boundary is still local journal durability. Cross-machine and branch boundaries depend on committed `MEMORY.md`.
- `summary` remains a one-line server response for both ops. For `add`, the summary is the fact.
- Concerto UI gate failed in this environment before UI test bootstrap, not in app assertions.

## What's Not Included

- Typed memory blocks.
- Forced externalization at land or context compaction.
- Cross-machine replay beyond committed `MEMORY.md`.

## Validation

- `cargo fmt --check` passed.
- `cargo clippy -- -D warnings` passed.
- `uv run python scripts/test.py` passed: Rust and website suites.
- `uv run python scripts/test.py --all` passed Python, Rust, website, Swift, and e2e; failed Concerto UI because `ConcertoUITests-Runner` was killed before establishing the XCTest connection and the app process hung. Result bundle: `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-dptljcblnlxvwdbazshrscvzqnra/Logs/Test/Test-Concerto-2026.07.06_14-37-19--0700.xcresult`.
