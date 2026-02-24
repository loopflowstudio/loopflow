# Agent API hardening + transcript + API harness review

## What was implemented

- Hardened session SSE semantics with an explicit replay boundary (`event: session.replay_completed`) and replay→live promotion in Concerto.
- Added restart/crash lifecycle recovery in `lfd`:
  - startup orphan recovery for `starting`/`active` sessions
  - abnormal harness exits now fail sessions and complete in-flight items as failed.
- Generalized in-flight item tracking in shared harness code and wired Claude/Codex through it.
- Expanded provider mapping + parity tests (Codex JSON-RPC, Claude NDJSON) and multi-client stream consistency checks.
- Expanded Swift shared session models + Concerto transcript state for typed items, bounded details, seq filtering, and reconnect/replay handling.
- Added hermetic HTTP API smoke infrastructure for wave CRUD and CI wrapper coverage.
- Added script-level session/fork smoke helpers and stabilized local fork/session test auth boot by:
  - booting out launchd-managed local `lfd` before test startup
  - using deterministic static auth tokens for test-owned daemons.

## Key choices

- **Protocol correctness over client heuristics**: replay completion is now an explicit SSE event type, not a timer guess.
- **Server-owned failure semantics**: restart/crash transitions are emitted by `lfd`, not inferred by clients.
- **Provider-generic crash handling**: in-flight tracking lives in shared harness code so all providers get consistent crash completion behavior.
- **Hermetic HTTP testing as foundation**: route assertions and daemon lifecycle live in reusable Python harness modules; shell wrappers stay thin.
- **Deterministic local auth in fork/session scripts**: test-owned daemons no longer depend on `~/.lf/session-token` races with launchd services.

## How it fits together

`lfd` now enforces replay/live and crash/restart invariants at the session core (store + harness + SSE). Swift models preserve typed fidelity, and Concerto reduces those typed events into one transcript path for replay and live updates. Python smoke harnesses validate the external HTTP contract end-to-end against live daemon instances.

## Risks and bottlenecks

- `xcodebuild test -scheme Concerto` still fails in this environment at `ConcertoUITests/ScreenshotPipelineTests.testCapture` (app activation/background state), though Swift package tests pass.
- Fork e2e still depends on Docker + Claude credentials; runtime behavior varies with local credential/container setup.
- Broadcast lag backfill is still deferred (`Lagged => continue`), so extremely slow consumers are not yet backfilled mid-stream.

## What's not included

- Broadcast lag backfill implementation.
- Rich transcript viewers (terminal emulation/diff rendering), turn grouping, multi-session picker UX.
- Full provider-layer unification beyond shared in-flight tracking.
- Additional API domains beyond current wave CRUD smoke coverage.

## Validation run

- `cargo fmt --all -- --check` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py -v` ✅
- `uv run pytest tests/e2e/test_fork.py -v` ✅
- `uv run python scripts/test_session.py --skip-build` ✅
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` ⚠️ fails at `ConcertoUITests/ScreenshotPipelineTests.testCapture`
