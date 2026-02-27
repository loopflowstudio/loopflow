# 02: Proactive Refresh

## Problem

`provider_tokens` are now persisted, but lfd still treats them as static. Long-running waves can hit silent auth expiry and fail mid-run. The users who benefit most are:

- unattended wave operators (overnight / weekend runs),
- container users (no host keychain fallback),
- fresh installs where auth should stay healthy after initial setup.

Why now: auth phase 01 and 03 are already shipped; without phase 02, the wave goal **"Token refresh happens without user intervention"** is still unmet.

## Approach

Ship a proactive refresh loop with provider-specific refresh adapters, strict time bounds, and explicit events.

1. Add `spawn_token_refresh(store, event_hub, cancel)` in `lfd/triggers/token_refresh.rs`.
   - Tick every 5 minutes.
   - On each tick: `list_provider_tokens()`, filter tokens where `expires_at < now + 20 minutes`.
   - Skip rows with `expires_at = None`.

2. Refresh due providers in parallel without stalling the loop.
   - Spawn one task per due provider.
   - Wrap each provider refresh in a 30-second `tokio::select!` timeout.
   - Continue processing other providers if one hangs or fails.

3. Add a per-provider singleflight lock.
   - `HashMap<Provider, Arc<tokio::sync::Mutex<()>>>`.
   - `try_lock` and skip if already in-flight, so overlapping ticks never race.

4. Add refresh adapters in `provider_auth.rs` (reuse existing extractors).
   - **GitHub**: run `gh auth refresh` (or host-scoped equivalent), then re-read `~/.config/gh/hosts.yml`.
   - **Claude**: no trusted refresh CLI path; re-read `~/.claude/.credentials.json`.
   - **Codex**: attempt `codex login --refresh`; always follow with re-read of `~/.codex/auth.json`.

5. Define refresh result contract.
   - Success: extracted token is present and no longer expired.
   - On success: upsert token, preserve prior `login` when extractor does not provide one, emit `Event::auth_token_refreshed`.
   - On failure: emit `Event::auth_refresh_failed` with reason.

6. Extend events.
   - Add `auth.token_refreshed` and `auth.refresh_failed` event variants + constructors in `types/event.rs`.
   - Keep auth event naming consistent with existing dotted auth event types.

7. Wire it into daemon startup.
   - Export in `triggers/mod.rs`.
   - Start loop from `Scheduler::start_loops`.

Research-based pattern choices:
- periodic sweep + due window (simple, robust),
- singleflight per provider (prevents refresh races),
- bounded async tasks with timeout (keeps scheduler healthy under CLI hangs).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Refresh only at executor launch | Simpler wiring; no background loop | Misses long-running runs; violates proactive refresh intent |
| Per-token exact scheduler (next-expiry heap) | Efficient timing and less polling | More state complexity for little gain with only 3 providers |
| Trigger polling + due window + per-provider locks (chosen) | Small periodic overhead | Best reliability/complexity ratio; aligns existing `spawn_X` trigger pattern |

## Key decisions

- **Decision: keep refresh orchestration in triggers, provider logic in `provider_auth`.** This keeps loop mechanics consistent with existing triggers and keeps provider CLI/file knowledge in one place.
- **Decision: timeout every provider refresh at 30s.** Prevents one hung CLI from blocking all future refresh work.
- **Decision: emit explicit success/failure events, not silent logs.** Supports UX prompts and observability when re-auth is required.
- **Decision: preserve filesystem fallback behavior.** Advances wave goal **"Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)."**
- **Wild success target:** users run lfd for days with zero manual re-auth; auth health is visible through periodic `auth.token_refreshed` events.
- **Wild failure guardrail:** provider credential format drift breaks extraction. Mitigation: defensive parsing + `auth.refresh_failed` events instead of panic; loop keeps running.
- **New risk introduced:** refresh storms across many lfd instances at same wall-clock boundary. Mitigation: singleflight locks locally and strict per-provider timeout.

## Scope

- In scope:
  - `spawn_token_refresh` trigger running every 5 minutes.
  - 20-minute refresh threshold selection logic.
  - Provider-specific refresh strategies (GitHub CLI + Claude/Codex file fallback).
  - Per-provider lock + timeout handling.
  - New auth refresh success/failure events.
  - Rust tests for refresh behavior and trigger non-blocking semantics.
- Out of scope:
  - Multi-user token isolation.
  - lfd-owned OAuth PKCE/device-flow implementation.
  - At-rest token encryption beyond current DB file permissions.
  - UI changes beyond consuming emitted events.

## Done when

- `cargo test -p loopflow token_refresh` passes with coverage for:
  - due-token refresh success path,
  - provider command missing/failing fallback path,
  - timeout path emitting failure event.
- `cargo test -p loopflow triggers` passes with loop continuity verified after per-provider failures.
- On a running lfd, a token expiring within 20 minutes produces either:
  - `auth.token_refreshed` + updated DB row, or
  - `auth.refresh_failed` without daemon crash/panic.
- This phase demonstrably advances wave goals:
  - **"Token refresh happens without user intervention"**
  - **"Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)"**
