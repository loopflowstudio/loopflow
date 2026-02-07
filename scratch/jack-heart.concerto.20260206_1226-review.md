# Review: Concerto Cleanup + Python API + lfd v1 Endpoints

## What was implemented

Three interlocking changes that move Concerto to a wave-only architecture:

1. **Concerto cleanup** — Deleted 13 services, 8 models, 3 views, 1 state object, and 1 protocol that existed to support worktree-level abstractions and the shelved step launcher UI. Concerto now talks to lfd exclusively via `LocalWaveService` (HTTP) and `LocalEventService` (WebSocket). ~3600 lines removed from Swift.

2. **Python API client** (`python/loopflow/`) — Pure-Python HTTP client replacing the PyO3 bindings (`rust/loopflow-py/`). Exposes `loopflow.api` module for wave CRUD, run management, and log streaming. CLI entry point at `lfq`. Build system switched from maturin to hatchling.

3. **lfd v1 endpoints** — Added `/v1/waves/{id}` (get single wave), `/v1/wave_runs` (list/filter runs), `/v1/flows` (list flows+steps), wave status updates via PATCH, run action endpoints (run/stop/land/continue). Postgres store gained baseline+incremental migration strategy and run snapshot support.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Delete WaveServiceProtocol | Only one implementation existed (LocalWaveService). Concrete type is simpler. | Keep protocol for testing — but SwiftUI previews use mock data directly. |
| Python httpx client over PyO3 | PyO3 required Rust compilation on install, breaking `uv tool install`. Pure Python is zero-compile. | Keep PyO3 — but install friction was too high for a thin HTTP wrapper. |
| WaveViewModel separate from Wave | Wave is the API model (lfd JSON). WaveViewModel adds display logic (statusIndicator, displayName, detailText). Keeps model pure. | Computed properties on Wave — but Wave is Sendable/Codable and shouldn't carry UI logic. |
| Baseline+incremental migrations | New databases get all baseline SQL at once (fast). Existing databases only run incremental migrations (safe). | Single migration list — but re-running idempotent baselines on existing DBs is fragile. |
| Event-driven wave updates | `handleWaveEvent` fetches only the affected wave on WS events instead of refreshing all waves. | Full refresh — but O(n) fetches on every event don't scale. |

## How it fits together

```
Concerto (SwiftUI)
  ├── RepoState ──→ LocalWaveService ──→ lfd /v1/waves, /v1/wave_runs, /v1/flows
  ├── SessionState ──→ LocalEventService ──→ lfd /ws (WebSocket)
  └── Views reference WaveViewModel (display) wrapping Wave (API model)

Python CLI (lfq)
  └── loopflow.api ──→ loopflow.client.Client ──→ lfd /v1/*

lfd
  ├── HTTP routes: waves.rs, wave_runs.rs, flows.rs
  ├── Store: sqlite.rs + postgres.rs (with migration strategy)
  └── WebSocket: ws.rs (events for live updates)
```

## Risks and bottlenecks

- **No Python tests yet.** The `python/loopflow/` package has no test coverage. The API is a thin wrapper over HTTP, but edge cases (connection errors, malformed responses) are untested.
- **Single-wave fetch on every WS event.** If many events fire rapidly (e.g., during a fork with parallel agents), each triggers a separate `getWave` HTTP call. Consider debouncing or batching.
- **Postgres migration strategy is new.** The baseline/incremental split hasn't been exercised in production. The empty `INCREMENTAL_MIGRATIONS` array means it's untested until the first real incremental migration lands.

## What's not included

- **Concerto UI test update.** The xcodegen-based UI tests aren't updated in this branch. They need a separate pass to remove references to deleted views.
- **Wave detail panel UX redesign.** The panel was simplified (removed worktree/PR actions) but not redesigned. It's functional but sparse.
- **Flow picker migration to /v1/flows.** The flow picker still hits the legacy `/flows` endpoint (now backed by the same handler). A follow-up should clean up the route.

## Gate fixes applied

| Fix | File |
|-----|------|
| Added `use std::str::FromStr` | `rust/lfd/src/http/routes/waves.rs` |
| Removed unused `WaveStatus` import | `rust/lfd/src/http/routes/mod.rs` |
| Prefixed unused `wave` param with `_` | `rust/lfd/src/http/routes/waves.rs:582` |
| Removed dead `MIGRATIONS` const (duplicate of `BASELINE_MIGRATIONS`) | `rust/lfd/src/store/postgres.rs` |
| Made `FlowsResponse` `pub(crate)` | `rust/lfd/src/http/routes/flows.rs` |
| Removed dead `wave_status_from_run` function | `rust/lfd/src/http/dto.rs` |
| Collapsed nested `if` (clippy) | `rust/loopflow-engine/src/agent.rs` |
| Fixed `parseWaveRunFromJSON` static method call | `swift/LoopflowCore/Services/LocalWaveService.swift` |
| Removed orphaned `StepRunJSON` init | `swift/LoopflowCore/Models/Step.swift` |
| Fixed test parameter ordering | `swift/ConcertoTests/WaveTests.swift` |
| Fixed `makeWave` scope in `WaitingReasonTests` | `swift/ConcertoTests/WaveTests.swift` |
| Added `dict[str, Any]` return types | `python/loopflow/api.py` |
| Added missing blank line before `wave_logs` | `python/loopflow/api.py` |
