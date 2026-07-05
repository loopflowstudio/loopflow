# DTO wire fixtures

Each fixture pins one wire shape. Most are mirrored across three hand-written
models — Rust (`rust/loopflow/tests/dto_fixtures.rs`), Python
(`python/tests/test_dto_fixtures.py`), Swift
(`swift/ConcertoTests/DTOFixtureTests.swift`) — so a drift in any mirror fails
one of the three suites. No serde/Pydantic/init defaults on DTOs: every absent
field is a parse error or an explicit null.

Carve-out: `resident_deltas.json` is the wave listener↔resident wire
(`POST /resident/deltas`, see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture test pins it — Swift and Python
do not consume this wire.
