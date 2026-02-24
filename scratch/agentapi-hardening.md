# 04: Hardening

## Problem

Session reliability is still "demo strong," not production strong. Slow SSE consumers can miss events, reconnect relies on a UI timer heuristic, active sessions become orphans across `lfd` restart, and harness crash behavior is inconsistent across providers.  
Who benefits: Concerto users, API clients, and wave automation that depends on trustworthy session state.  
Why now: this is the last reliability gate before adding more providers and reusing this layer for both CLI and HTTP surfaces.

## Approach

Ship a reliability-first hardening pass centered on one rule: **no silent event loss**.

1. **Make replay/live boundary explicit**
   - Add a server event `ReplayCompleted { last_seq }` on `/v0/sessions/{id}/events`.
   - Stream order becomes: replay from store → `ReplayCompleted` → live follow.
   - Concerto can promote reconnect state on sentinel, not timer fallback.

2. **Backfill on broadcast lag instead of skipping**
   - Keep `tokio::broadcast` for fan-out.
   - On `Lagged`, fetch missing `[expected_seq, newest_seq)` from store and emit them in-order before resuming live stream.
   - Preserve strictly increasing seq per client stream.

3. **Harden runtime lifecycle across restart/crash**
   - On `lfd` startup, mark persisted `starting`/`active` sessions as `failed` with a recovery error event (`lfd_restarted_orphaned_session`).
   - In bridge tasks, treat unexpected harness death as terminal session failure (provider-specific rules for Codex vs Claude normal exits).
   - Keep end-session idempotent, including end during `starting`.

4. **Make Claude abnormal exits explicit in transcript**
   - Track in-flight tool items.
   - On abnormal process exit, emit `ItemCompleted(Failed)` for each in-flight tool with crash metadata instead of dropping partial tool state.

5. **Prove invariants with contract-style tests**
   - Add provider trace replay tests (Codex JSON-RPC + Claude NDJSON fixtures).
   - Add multi-client SSE tests validating identical item IDs and ordered seqs across clients.
   - Add restart recovery and lag-backfill integration tests.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep timer-based reconnect promotion and current lag skip behavior | Minimal code churn | Leaves silent data-loss edge cases and reconnect ambiguity in core path |
| Replace broadcast with per-client durable queues | Strong reliability semantics | Too heavy for this phase; large memory/complexity jump and slower ship |
| **Chosen: broadcast + store backfill + replay sentinel** | Moderate implementation complexity | Best reliability gain per line of code; keeps current architecture intact |

## Key decisions

1. **Adopt an explicit replay boundary event now.** We are removing timer heuristics from correctness paths.
2. **Treat lag as recoverable, not fatal.** Missing events are backfilled from persisted store in-stream.
3. **Crash state is server-owned.** `lfd` always emits terminal failure semantics for orphaned or crashed sessions.
4. **Follow wave principles directly:**
   - "**Harness-first, not protocol-first.**" Reliability work stays in harness/runtime correctness, not client workarounds.
   - "**lfd owns the session lifecycle.**" Restart/crash recovery is solved in `lfd`, not delegated to Concerto.
   - "**Reconnect replays persisted events then follows live stream.**" Sentinel + lag backfill make this invariant mechanically true.

## Scope

- In scope:
  - `ReplayCompleted` sentinel event in session SSE protocol
  - Lagged receiver store backfill path
  - Startup orphan recovery for `starting`/`active` sessions
  - Provider crash-to-failed-session transitions
  - Claude in-flight tool failure completion on abnormal exit
  - Concurrent-client and provider-trace conformance tests
  - Verify end-session behavior during `starting`
- Out of scope:
  - New diff visualization UX in Concerto
  - Multi-session picker/history UI
  - Re-architecting away from `tokio::broadcast`
  - Full provider-layer unification (phase 07)

## Done when

- A slow SSE client that overruns broadcast capacity reconnects/continues with no seq gaps (validated by integration test).
- Concerto reconnect promotion depends on `ReplayCompleted`, not timer fallback.
- Restarting `lfd` marks previously `starting`/`active` sessions as `failed` with explicit error events.
- Unexpected provider process death transitions session to `failed` and closes in-flight Claude tools as `Failed`.
- Two concurrent SSE clients observe stable `item.id` values and monotonic seq ordering for the same session.
- `cargo test --all`, `uv run pytest python/tests/`, and `uv run pytest tests/e2e/test_api_smoke.py -v` remain green after hardening changes.
