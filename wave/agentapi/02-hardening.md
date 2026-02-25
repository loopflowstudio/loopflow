# Hardening

Status: **mostly shipped** (Phase 04, branch `jack-heart.agentapi.20260225_1122`)

## What shipped

Hardening moved forward before the OpenCode adapter — the reliability gaps were blocking real session use and didn't depend on a third transport.

- **Process crash recovery**: Claude crash handling drains the reader, avoids stale completion races, and completes in-flight tool items as failed on abnormal exit. Codex EOF already worked.
- **Event loss prevention**: replaced harness→bridge delivery with unbounded `mpsc` so events are never dropped before store persistence.
- **SSE lag recovery**: store-backed backfill on `RecvError::Lagged` — turned out to be essential, not polish.
- **Conformance replay tests**: Claude (normal, crash mid-tool, multi-tool) and Codex (normal, error) with recorded traces. Established the pattern for OpenCode.
- **Wave integration (partial)**: session lifecycle wired into scheduler occupancy tracking (register on create, unregister on terminal). Advancement is tick-driven via scheduler loop, not immediate callback.
- **Starting-state stop handling**: sessions can fail fast without hanging on incomplete startup. Stop-during-starting cleans up harness processes.

## What's left

- **lfd restart / orphan cleanup**: `SessionRuntime` still lives in memory only. Active sessions become orphans on restart. Events survive in the store. Startup recovery pass needed: mark orphaned `active`/`starting` sessions as `failed`.
- **Immediate wave advancement**: open question whether successful session completion should trigger immediate waiting-step advancement or remain tick-driven. Current choice (tick-driven) is simpler but adds latency.

## What we learned

- SSE lag recovery and conformance tests were originally listed as "not worth doing separately." Both turned out to be essential — lag recovery for correctness under real reconnect patterns, conformance tests for catching event mapping bugs that only appear with specific provider output sequences.
- Claude's reader-task-stop race was more nuanced than expected. Draining the reader before shutdown and handling stop-during-starting required careful state machine work.
- Unbounded `mpsc` is the right call for harness→bridge. Bounded channels risk subprocess backpressure, which is worse than memory growth.

## Done when

- ~~Dead harness processes detected and session marked `failed`~~ ✓
- lfd restart doesn't leave orphaned active sessions
- ~~Session end advances wave run~~ ✓ (tick-driven)
