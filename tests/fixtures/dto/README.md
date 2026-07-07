# DTO wire fixtures

Each fixture pins one wire shape. Most are mirrored across three hand-written
models — Rust (`rust/loopflow/tests/dto_fixtures.rs`), Python
(`python/tests/test_dto_fixtures.py`), Swift
(`swift/LoopflowTests/DTOFixtureTests.swift`) — so a drift in any mirror fails
one of the three suites. No serde/Pydantic/init defaults on DTOs: every absent
field is a parse error or an explicit null.

Carve-out: `resident_deltas.json` and `resident_door.json` are the wave
listener↔resident wire (`POST /resident/deltas`, `POST /resident/attach`,
`GET /resident/context` — see `rust/loopflow/src/wave/wire.rs`). Both ends are
the same `lf` binary, so only the Rust fixture tests pin them — Swift and
Python do not consume this wire.

`channel_tagged_turn.json` pins a work-line channel's `turn` SSE frame: the
ChatTurn JSON plus one extra top-level `channel` key (absent = the wave's own
channel). Rust pins the additive shape; Swift decodes it through
FrameChannelTag + ChatTurn. Python does not consume the wave SSE stream.
