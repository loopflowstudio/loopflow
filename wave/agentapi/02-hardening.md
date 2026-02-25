# Hardening

Status: **mostly shipped** (Phase 04, branch `jack-heart.agentapi.20260225_1122`)

Production failure modes that cause stuck or lost sessions. The OpenCode adapter shipped, completing the three-harness validation. This phase builds on the simplified session codebase.

## What shipped

Hardening moved forward before the OpenCode adapter — the reliability gaps were blocking real session use and didn't depend on a third transport. OpenCode adds a few harness-specific concerns: SSE disconnect is already terminal (`opencode_disconnected`), but HTTP request failures during a turn and orphaned `opencode serve` processes on lfd restart need the same treatment.

- **Process crash recovery**: Claude crash handling drains the reader, avoids stale completion races, and completes in-flight tool items as failed on abnormal exit. Codex EOF already worked.
- **Event loss prevention**: replaced harness→bridge delivery with unbounded `mpsc` so events are never dropped before store persistence.
- **SSE lag recovery**: store-backed backfill on `RecvError::Lagged` — turned out to be essential, not polish.
- **Conformance replay tests**: Claude (normal, crash mid-tool, multi-tool) and Codex (normal, error) with recorded traces. Established the pattern for OpenCode.
- **Wave integration (partial)**: session lifecycle wired into scheduler occupancy tracking (register on create, unregister on terminal). Advancement is tick-driven via scheduler loop, not immediate callback.
- **Starting-state stop handling**: sessions can fail fast without hanging on incomplete startup. Stop-during-starting cleans up harness processes.

## What's left

- **lfd restart / orphan cleanup**: `SessionRuntime` still lives in memory only. Active sessions become orphans on restart. Events survive in the store. Startup recovery pass needed: mark orphaned `active`/`starting` sessions as `failed`. OpenCode adds orphaned `opencode serve` processes.
- **Immediate wave advancement**: open question whether successful session completion should trigger immediate waiting-step advancement or remain tick-driven. Current choice (tick-driven) is simpler but adds latency.
- **OpenCode conformance replay tests**: the adapter shipped with unit tests for event mapping but no recorded-trace replay tests. Especially important given the inferred event schema (see `scratch/questions.md`).

## What we learned

- SSE lag recovery and conformance tests were originally listed as "not worth doing separately." Both turned out to be essential — lag recovery for correctness under real reconnect patterns, conformance tests for catching event mapping bugs that only appear with specific provider output sequences.
- Claude's reader-task-stop race was more nuanced than expected. Draining the reader before shutdown and handling stop-during-starting required careful state machine work.
- Unbounded `mpsc` is the right call for harness→bridge. Bounded channels risk subprocess backpressure, which is worse than memory growth.

## What the adapter taught us

- The session codebase was significantly simplified alongside the adapter (sessions/mod.rs -300 lines, flow.rs -130 lines, conformance tests removed). Hardening builds on this cleaner base.
- OpenCode's HTTP+SSE transport is the odd one out — the other two harnesses use stdio. Process management and failure detection work differently: OpenCode crash = HTTP error or SSE disconnect, not process EOF.
- OpenCode event schema is inferred from observation, not from a spec. Two open questions remain about `POST /session` response shape and `ToolPart` payload fields. Conformance tests that record real OpenCode traces would close both.

## Done when

- ~~Dead harness processes detected and session marked `failed`~~ ✓
- lfd restart doesn't leave orphaned active sessions or orphaned `opencode serve` processes
- ~~Session end advances wave run~~ ✓ (tick-driven)
- Conformance replay tests pass for all three harnesses with recorded traces
