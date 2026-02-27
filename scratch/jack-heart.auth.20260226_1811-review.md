# Auth Phase 02: Proactive Token Refresh — Review

## What was implemented

Background token refresh loop that detects tokens approaching expiry and refreshes them proactively. Covers GitHub (CLI refresh + file re-read), Claude (file re-read), Codex (CLI attempt + file fallback), and OpenCode Zen (file re-read). Emits `auth.token_refreshed` / `auth.refresh_failed` events for observability.

Additionally: expiry parsing across all provider extractors (epoch seconds, millis, RFC3339), OpenCode Zen auth broker (device flow, credential extraction, env injection, disconnect), and socket broker token extraction with expiry awareness.

## Key choices

- **Trigger pattern, not exact scheduler.** A 5-minute polling loop with a 20-minute due window. Simple, matches existing `spawn_X` trigger conventions. Marginally wasteful for 3-4 providers; dramatically simpler than a per-token expiry heap.

- **Singleflight per provider.** `try_lock()` skips instead of queuing, so overlapping ticks never race. One stuck provider can't starve others.

- **30-second timeout per provider refresh.** A hung `gh auth refresh` won't block the loop. Emits failure event and moves on.

- **Separate `TokenRefresher` / `RefreshCommandRunner` traits for testing.** These are private traits that enable clean test injection without real CLI calls or filesystem I/O. Tests cover success, failure, timeout, and cross-provider isolation.

- **Login preservation.** When a refreshed token doesn't include a login (common for file-based extraction), the prior login is carried forward.

- **Expiry normalization.** Handles epoch seconds, epoch millis (>100B threshold), RFC3339 strings, and floats. Defensive — returns `None` on anything unparseable rather than panicking.

## How it fits together

```
Scheduler::start_loops
  └── spawn_token_refresh(store, event_hub, cancel)
        └── every 5 min: schedule_due_refreshes
              ├── list_provider_tokens from DB
              ├── filter: expires_at < now + 20min, skip None
              └── per due provider (parallel):
                    ├── try_lock (skip if in-flight)
                    ├── timeout(30s, refresh_provider_token(provider))
                    ├── validate: refreshed token not already expired
                    ├── preserve prior login if missing
                    ├── upsert to DB
                    └── emit auth.token_refreshed / auth.refresh_failed
```

Provider refresh logic stays in `provider_auth.rs`. Loop orchestration stays in `triggers/token_refresh.rs`. Events defined in `types/event.rs`.

## Risks and bottlenecks

- **Provider credential format drift.** All extractors parse defensively and return `None` on unexpected formats. The refresh loop emits `auth.refresh_failed` and continues — never panics.

- **`codex login --refresh` may not exist.** The Codex refresh path swallows command failure and falls back to file re-read. This matches the design doc.

- **Refresh storms.** Multiple lfd instances sharing a home directory could all attempt refresh at the same wall-clock boundary. Singleflight locks only protect within a single process. Mitigation: the 30s timeout bounds blast radius.

- **OpenCode Zen tokens have no expiry.** `expires_at` is typically `None`, so the proactive loop skips them. This is correct per design — the loop focuses on tokens that *can* expire.

## What's not included

- Multi-user token isolation (out of scope for auth wave)
- lfd-owned OAuth PKCE/device-flow implementation
- Token encryption at rest
- UI changes beyond consuming emitted events
- Jitter/randomization on refresh timing (mitigates cross-instance storms but adds complexity for minimal gain with current scale)
