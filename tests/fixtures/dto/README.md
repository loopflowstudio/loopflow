# DTO wire fixtures

Each fixture pins one live wire shape. Swift fixtures cover the per-Wave
listener and `lf status` contracts consumed by the Mac app. Every absent field
is a parse error or an explicit null.

Carve-out: `resident_deltas.json` and `resident_door.json` are the wave
listener↔resident wire (`POST /resident/deltas`, `POST /resident/attach`,
`GET /resident/context` — see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture tests pin them. Swift does not
consume this wire.

`receipt.json` is one `MemoryFact` (the `lf memory log --json` shape): a curated
fact plus its evidence `receipts`, covering every `EvidenceKind`. Pinned in Rust
(`memory_fact_fixture_round_trips_every_evidence_kind`) and Swift
(`memoryFactFixturePreservesReceipts`).

`interactive_handoff_attach.json` is the store-direct `lf handoff attach --json`
shape. It carries durable identity and structured presentation instructions;
terminal bytes are deliberately absent. Rust and Swift both round-trip it.

`interactive_handoff_list.json` is the `lf handoff list --json` census shape: a
row per durable handoff with identity, declared parent, Home, provider, reason,
and age, but no argv or environment. Two rows exercise both parent kinds, both
active statuses, an absent provider session, and an unreadable age (`null`, never
a fabricated zero). Rust and Swift both round-trip it.

`active_sessions_census.json` is the composed input the Mac's `ActiveSessionsCensus`
projects — `{roadmap, runs, handoffs}` in one file (Swift-only; ids are opaque
strings here, not round-tripped through Rust's typed id parsers). It packs one
scenario per rule: red propagation from a waiting handoff up through Task,
Project, and Wave; observed/stale/stopped/unavailable/unreachable evidence;
`unavailable` vs. `missing` empty states; a live vs. finished direct-execution
run; a filtered completed handoff and unstarted task; and an orphan handoff whose
Wave is absent from the roadmap. `ActiveSessionsCensusTests` asserts the projection.

`task_attention_states.json` pins the Rust-owned desktop attention fold for
live advancing, live human wait, dead dirty, dead authored commits, clean
backlog, completed, stale active intent, and unavailable local evidence. Rust
and Swift decode the same Task rows; consumers never reconstruct the signal
from process flags.

`context_lab_snapshot.json` is shared by Rust and Swift. It pins the atomic
session-set query, including explicit missing token coverage and immutable trace
addresses. Revision evidence carries both Rust's effective-content hash and the
current source-file hash used to reject a stale Task worktree without
reimplementing prompt transformations in Swift.
