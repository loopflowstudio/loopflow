# W2-135 — leased, progress-aware, recoverable Session bodies (serial PR4+)

The three merged PRs proved slices, not the broad contract:

- #898 (PR1) — `BodyLease`/`BodyObservation` types + `observe()` projection +
  `body_intent()` on both statuses. **Built but never wired**: `observe()` had
  zero non-test callers; the runtime snapshot still shipped `{status,
  process_alive}`.
- #901 (PR2) — provider-body write lease across handoffs.
- #903 (PR3) — process-group lease ownership + reap + recovery **mechanics**.

## Contract status (ground truth, not PR summaries)

| Done-when | Before PR4 | After PR4 |
|-----------|-----------|-----------|
| Progress-aware stall detection | projection only, no caller, no real progress signal | **live**: producer derives `progress_age` from the durable event log; wedged body reads Stalled on every surface |
| Process-group lease ownership | IMPLEMENTED (#903) | unchanged |
| Safe same-Session recovery preserving work + PR history | mechanics done; not driven by stall; Uncertain→NeedsInput is command-level only | mechanics unchanged; **PR5** drives it from the stall observation and links Uncertain→NeedsInput at the Session-status level |
| Shared DTO + status consumers | ABSENT (runtime snapshot = `{status, process_alive}`) | **DONE**: `observation` rides `TaskRuntimeSnapshot`/`ProjectRuntimeSnapshot`, mirrored in Swift, round-trip fixtures + assertions |
| Real dogfood recovery | one reap+gen+1 unit test; no stall path, no provider | **PR5** |

## PR4 (this branch) — shared observation on the wire

- Store: `latest_task_event_at` / `latest_project_event_at` (`SELECT MAX(created_at)`)
  — the honest progress signal (events append but don't bump `updated_at`; a body
  silent in the event log past the deadline is the >4h-sleep incident).
- `lf/commands/waves.rs`: `progress_age()` + both runtime producers build
  `BodyEvidence` and call `observe()`; `observation` added to both snapshots.
- Swift `WaveWorkMap.swift`: `BodyObservation` + `BodyCategory`/`BodyOwner`/`BodyControl`
  mirror; `observation` on both snapshots.
- Fixtures: coherent `observation` on every runtime block across the four DTO
  fixtures + Mac mock; Stalled pinned on `ts_human`/`ts_stale`. Rust + Swift
  assertions pin Working / Stalled / NeedsInput on the wire.

Gate: `cargo test -p loopflow` (1280), clippy/fmt clean, `swift test` (56).

## PR5 (next) — recovery driven by the observation + dogfood

1. Parent loop (Project wake for Tasks; Wave resident for Projects) reads the
   `observe()` result; a body that reads **Stalled** past its deadline is revoked,
   its process group reaped, and gen+1 started on the same Session — reusing the
   #903 `revoke_and_reap_child_body` + `begin_generation` mechanics.
2. A body lost mid-`Delivering`/`Uncertain` drives the Session to a status whose
   observation is **NeedsInput** — no replay (link `mark_stale_child_deliveries_
   uncertain` to Session status, not just the command).
3. Terminal / open-PR still bars restart (`supervisor_restart_bar`).
4. Deterministic dogfood: fake provider stalls under an injected clock → observed
   Stalled → recovered on the same Session/worktree/provider history before a human
   would notice from wall-clock.

Then compare the entire contract before proposing completion.
