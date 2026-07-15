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
