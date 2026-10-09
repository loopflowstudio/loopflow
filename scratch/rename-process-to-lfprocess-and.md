# LOO-441: ownership names

Jack Heart accepted the pure rename on 2026-10-09: Loopflow owns LfSession
and LfProcess; provider-owned names belong to the next Task. One PR, no behavior,
wire, storage, fixture, or golden changes. The supplied Task brief is the design.

## Delete — do not maintain

- Replace the old conversation type and all live references with LfSession.
- Replace process.rs's Process and its five filter/page/cursor types with the
  LfProcess family; preserve ProcessLfid, FlowProcess*, receipts and enum variants.
- Replace Swift's model file with LfProcess.swift and move its consumers.
  Foundation's process no longer needs qualification.
- Remove the architecture sentence forbidding the future provider process name.
  Keep product-facing Process/Session and prose Process unchanged.

## Remaining

The rename is implemented across Rust, Swift and live documentation. Wire fields,
SQL, fixture bytes and enum variants are unchanged. Gate owns full Rust tests and
automated acceptance; delivery follows the authored Flow.

Review: the compiler and remaining-token audit distinguish the model from enum
variants and Foundation processes. No aliases or second owners remain. Rustfmt
collapses three shortened signatures, accounting for ten removed lines beyond
the defining-sentence edits; do not add filler to force balanced diff counts.

Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `swift build --package-path swift`, network-isolated Rust `dto_fixtures` (22) and Swift `DTOFixtureTests` (23), and README/index sync (1) pass; full Rust tests remain with gate. Swift tests use `--disable-sandbox` inside the network wrapper to avoid nested sandbox denial.
