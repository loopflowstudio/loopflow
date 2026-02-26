# 02: Proactive Refresh

Background task that refreshes OAuth tokens before they expire.

## What to build

### spawn_token_refresh trigger

Follow the `spawn_X(store, cancel)` pattern in `triggers/`. Interval: 5 minutes. Refresh threshold: 20 minutes before expiry.

Each tick:
1. `list_provider_tokens()` from the store
2. For each token where `expires_at < now + 20min`:
   - Attempt refresh via provider CLI (`gh auth refresh`, `claude auth refresh`, or re-extract from credential files)
   - On success: upsert updated token, emit `Event::auth_token_refreshed`
   - On failure: emit `Event::auth_refresh_failed` (UI can prompt re-auth)
3. Skip tokens with no `expires_at` (no expiry known)

### Per-provider refresh strategy

- **GitHub**: `gh auth refresh` exists. Run it, re-extract token from `hosts.yml`.
- **Claude**: No known `claude auth refresh` command. Re-read `~/.claude/.credentials.json` — Claude Code may have refreshed the file itself. If the token is still expired, emit `auth.refresh_failed`.
- **Codex**: Try `codex login --refresh` or re-read `~/.codex/auth.json`. Same fallback pattern as Claude.

### Concurrency guard

Hold a per-provider lock during refresh to prevent races if the trigger fires while a previous refresh is in-flight.

## Constraints

- Don't block the trigger loop on a single provider's refresh. Use `tokio::select!` with a per-provider timeout (30s).
- Providers without a refresh CLI path get the file-re-read fallback, not a hard failure.

## Validation

```bash
cargo test -p loopflow token_refresh
cargo test -p loopflow triggers
```

## Done when

- Background task starts with lfd and ticks every 5 minutes
- Tokens within 20 minutes of expiry are refreshed
- Events emitted for success and failure
- No panics when a provider's refresh path doesn't exist
