# 01: Proactive Refresh

**Finish line:** Background task refreshes tokens before expiry. `lfq auth status` shows "refresh in Xm" that counts down, and tokens stay `active` through refresh cycles without user intervention.

## What to build

A background task in lfd that:

1. Watches `provider_tokens` for rows with `expires_at` set
2. Fires refresh before expiry (currently `TOKEN_REFRESH_LEAD_SECONDS = 20 * 60`)
3. Calls the provider CLI's refresh command (`gh auth refresh`, equivalent for Claude/Codex)
4. Updates the DB row with new token + expiry on success
5. Emits `auth.refresh_required` event when CLI-based refresh isn't supported or fails

The `next_refresh_at` field already exists in `ProviderAuthSnapshot` and flows through to `lfq auth status` — but it's currently a static estimate (`expires_at - 20min`), not driven by the actual scheduler. Wire these together.

## Context from shipped work

- `TokenStore::get_provider_token` returns `ProviderToken { token, login, expires_at }` — the background task reads this to find tokens approaching expiry.
- `ProviderAuthSnapshot` already carries `expires_at` and `next_refresh_at` (added in install-onboarding). The API serializes these as ISO 8601 strings; the Python CLI formats them as relative deltas.
- Install onboarding caps auth polling to 5 minutes per provider. The refresh task has no such constraint — it runs continuously.

## Open questions

- `gh auth refresh` exists. What's the equivalent for Claude and Codex? If there's no CLI command, fall back to re-running the device flow and emitting an event.
- Per-provider lock during refresh to prevent concurrent refresh races.
- Should refresh failure trigger a user notification beyond the event?

## Constraints

- Don't block the async runtime — refresh is a background `tokio::spawn` task.
- Graceful degradation: if a provider doesn't support refresh, log a warning and emit the event. Don't crash.
- Filesystem-based auth tokens (fallback path) don't get refreshed — they're managed by the provider CLI directly.
