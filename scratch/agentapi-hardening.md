# Agent API reliability hardening (current state)

## Scope
This is the canonical scratch doc for the session reliability + transcript + API smoke work on this branch.

## What is now true

### Session streaming correctness
- Session streams replay persisted events first, then emit `event: session.replay_completed`, then follow live events.
- Concerto promotes replay → live on the sentinel event (not timer heuristics).
- `StreamPhase.reconnecting` was replaced with `.replaying` semantics in chat state.

### Server-owned lifecycle recovery
- On `lfd` startup, persisted `starting`/`active` sessions are recovered to failed with an explicit recovery error event.
- Unexpected harness death is treated as terminal session failure.
- End-session remains idempotent, including end during `starting`.

### Provider-generic crash handling
- Shared `InFlightItems` tracking in `harness/common.rs` is used by Claude and Codex.
- On abnormal provider exit, in-flight items are drained and completed as failed with crash metadata.

### Transcript/model parity
- Rust typed session events map through shared Swift models with typed item/delta coverage.
- Concerto transcript state uses stable server item IDs, seq filtering, and bounded detail buffers.

### Hermetic API smoke baseline
- `scripts/lib/lfd_runtime.py` + `scripts/lib/api_harness.py` provide reusable live-HTTP assertions.
- `scripts/test_api_smoke.py` + `tests/e2e/test_api_smoke.sh` cover wave CRUD happy/error paths in CI.

### Local dev/test reliability
- Session/fork smoke scripts boot out launchd-managed `com.loopflow.lfd` before starting test-owned daemons.
- Test-owned daemons use deterministic static tokens to avoid `~/.lf/session-token` races.

## Validation snapshot
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

## Remaining work
1. Implement slow-consumer lag backfill (`tokio::broadcast` lag currently drops in-stream events).
2. Stabilize `ConcertoUITests/ScreenshotPipelineTests.testCapture` so full Concerto UI suite is green in this environment.
3. Extend hermetic API coverage beyond wave CRUD (chords, stimuli, run lifecycle, and stream/webhook surfaces).
4. Further harden fork e2e isolation (optional janitor + lockfile, or move to hermetic runtime where feasible).

## Decisions to preserve
1. `session.replay_completed` remains a protocol-level SSE event type, not a persisted `SessionEvent` variant.
2. Crash and orphan failure semantics are emitted by `lfd`, not inferred by clients.
3. In-flight crash completion behavior stays provider-generic in shared harness code.
4. Shared Swift models keep typed fidelity; Concerto flattens only at UI reduction boundaries.
