# W2-135 open questions / blockers

## PR1 is paused on W2-134 landing (by supervisor directive)

**Done this pass (non-overlapping additive core, committed `160a8c4ca`):**
- `BodyObservation` shared state model (category / reason / owner / controls /
  progress age / deadline) + `BodyCategory`, `BodyOwner`, `BodyControl`,
  `BodyIntent`, `BodyEvidence` in `child_session.rs`.
- `observe()` pure clock-free projection; `body_intent()` on
  `TaskSessionStatus`/`ProjectSessionStatus` so one projection serves both.
- 9 projection unit tests (Working/Stalled boundary/Stopped-not-gone/
  Unobservable/NeedsInput/Terminal/Failed). `cargo clippy --all-targets -D
  warnings` clean.

**Deferred until W2-134 lands and this worktree runs `lf rebase` onto it:**
Wiring `observe()` onto the serialized `TaskRuntimeSnapshot`/
`ProjectRuntimeSnapshot` requires adding a required `observation` field, which
breaks the shared `wave_detail` DTO fixture round-trip in **both** Rust
(`dto_fixtures`) and Swift (`DTOFixtureTests.swift`) — i.e. it forces the shared
DTO fixture registry + Swift decoder integration the coordination boundary says
to hold. Assumption: this is the correct stop point rather than making
`observation` artificially `Optional` to dodge the fixture (that would be the
"default masquerading" the DTO rule forbids).

**Resume plan when W2-134 is merged:** `lf rebase` onto the landed live-turn
contract, then in this same Task/worktree layer the additive wire:
`observation` on the two runtime snapshots (populated in `waves.rs` via
`observe()` + last-event progress age), `WaveWorkMap.swift` mirror, `wave_detail`
fixture update, Rust + Swift round-trip. Then PR2 = atomic write-lease +
process-group ownership (migration `0.11.003`).

No PR pushed/opened yet: the slice is intentionally paused on the external
W2-134 dependency and there is no reviewer in this headless run.
