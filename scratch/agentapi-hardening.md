# 04: Hardening

## Problem

Session reliability is still "demo strong," not production strong. Slow SSE consumers can miss events, reconnect relies on a UI timer heuristic, active sessions become orphans across `lfd` restart, and harness crash behavior is inconsistent across providers.
Who benefits: Concerto users, API clients, and wave automation that depends on trustworthy session state.
Why now: this is the last reliability gate before adding more providers and reusing this layer for both CLI and HTTP surfaces.

## Approach

Ship a reliability-first hardening pass centered on one rule: **no silent event loss**.

1. **Make replay/live boundary explicit**
   - Emit `event: session.replay_completed` with `data: {"last_seq": N}` as a separate SSE event type (not a `SessionEvent` variant).
   - Stream order becomes: replay from store → `session.replay_completed` → live follow.
   - Add `event:` field parsing to the Swift SSE stream reader in `LocalWaveService`.
   - Concerto promotes `StreamPhase.replaying` → `.live` on sentinel, replacing the 1s timer fallback.
   - Rename `StreamPhase.reconnecting` → `.replaying` throughout ChatState.

2. **Harden runtime lifecycle across restart/crash**
   - On `lfd` startup (before HTTP server accepts connections), mark persisted `starting`/`active` sessions as `failed` with a recovery error event (`lfd_restarted_orphaned_session`).
   - In bridge tasks, treat unexpected harness death as terminal session failure (provider-specific rules for Codex vs Claude normal exits).
   - Keep end-session idempotent, including end during `starting`.

3. **Generalize in-flight item tracking across providers**
   - Add `InFlightItems` tracker in `harness/common.rs` — insert on `ItemStarted`, remove on `ItemCompleted`, no-op on remove-of-unknown.
   - On abnormal process exit (any provider), drain tracker and emit `ItemCompleted(Failed)` for each in-flight item with crash metadata.
   - Both Claude and Codex harnesses use the shared tracker. Future providers (OpenCode) get it for free.

4. **Prove invariants with contract-style tests**
   - Add provider trace replay tests (Codex JSON-RPC + Claude NDJSON fixtures).
   - Add multi-client SSE tests validating identical item IDs and ordered seqs across clients.
   - Add restart recovery integration tests.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep timer-based reconnect promotion and current lag skip behavior | Minimal code churn | Leaves silent data-loss edge cases and reconnect ambiguity in core path |
| Replace broadcast with per-client durable queues | Strong reliability semantics | Too heavy for this phase; large memory/complexity jump and slower ship |
| Broadcast lag backfill from store on `Lagged` | Recovers missed events in-stream | Deferred — current `Lagged => continue` is acceptable until concurrent slow clients are a real problem |
| `ReplayCompleted` as `SessionEvent` enum variant | Simpler — same SSE event type | It's a connection property, not a session event; shouldn't be persisted or replayed |
| `ReplayCompleted` as `session.event` with special `type` field | No Swift SSE parser changes | Masquerades protocol event as session event; better to teach the parser about event types once |
| In-flight item tracking scoped to Claude only | Less code | Codex has the same crash gap; generalizing to `common.rs` costs little and covers all current + future providers |

## Key decisions

1. **Adopt an explicit replay boundary event now.** We are removing timer heuristics from correctness paths.
2. **`ReplayCompleted` is a separate SSE event type** (`event: session.replay_completed`), not part of the `SessionEvent` enum. Two concurrent clients each get their own sentinel at different points.
3. **Crash state is server-owned.** `lfd` always emits terminal failure semantics for orphaned or crashed sessions.
4. **In-flight item tracking is provider-generic.** Lives in `common.rs`, used by all harnesses.
5. **Orphan recovery runs before HTTP server starts.** No race between client reads and recovery writes.
6. **Lag backfill deferred.** `Lagged => continue` stays for now. The broadcast + store backfill approach is the right fix when needed, but not worth the complexity yet.
7. **Follow wave principles directly:**
   - "**Harness-first, not protocol-first.**" Reliability work stays in harness/runtime correctness, not client workarounds.
   - "**lfd owns the session lifecycle.**" Restart/crash recovery is solved in `lfd`, not delegated to Concerto.
   - "**Reconnect replays persisted events then follows live stream.**" Sentinel makes this invariant mechanically true.

## Scope

- In scope:
  - `session.replay_completed` SSE event in session streaming protocol
  - `event:` field parsing in Swift SSE stream reader
  - `StreamPhase.reconnecting` → `.replaying` rename in Concerto
  - Startup orphan recovery for `starting`/`active` sessions (pre-HTTP-bind)
  - Provider-generic `InFlightItems` tracker in `harness/common.rs`
  - Provider crash-to-failed-session transitions (Claude + Codex)
  - Concurrent-client and provider-trace conformance tests
  - Verify end-session behavior during `starting`
  - Concerto sentinel-based replay promotion
- Out of scope:
  - Broadcast lag backfill (deferred)
  - New diff visualization UX in Concerto
  - Multi-session picker/history UI
  - Re-architecting away from `tokio::broadcast`
  - Full provider-layer unification (phase 07)

## Done when

- Concerto reconnect promotion depends on `session.replay_completed` sentinel, not timer fallback.
- Restarting `lfd` marks previously `starting`/`active` sessions as `failed` with explicit error events.
- Unexpected provider process death transitions session to `failed` and emits `ItemCompleted(Failed)` for each in-flight item (validated for both Claude and Codex).
- Two concurrent SSE clients observe stable `item.id` values and monotonic seq ordering for the same session.
- `cargo test --all`, `uv run pytest python/tests/`, and `uv run pytest tests/e2e/test_api_smoke.py -v` remain green after hardening changes.
