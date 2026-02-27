# Auth

## Vision

lfd becomes a real OAuth token broker. Tokens in the DB, proactive refresh, credential injection into executors, guided onboarding at install. Not multi-user token isolation, not lfd implementing OAuth PKCE directly, not token encryption beyond filesystem permissions.

## Strategy

Today lfd delegates auth entirely to provider CLIs and infers status from filesystem heuristics. This works on a developer laptop. It breaks in containers (no keychain, no host filesystem), on fresh machines (`lfd install` finishes without connecting any providers), and during long-running waves (tokens expire mid-run, no refresh).

The fix: DB-backed token management. Tokens flow into lfd's database once at login. A background task refreshes them before expiry. Executors pull from the DB at launch — no host mounts, no env vars assumed from the host shell. Filesystem heuristics stay as fallback for existing installs.

Token store (Phase 01) and credential injection (Phase 03) shipped — `provider_tokens` table, `TokenStore` trait, per-provider extraction after auth flow, DB-first `check_status`, and `provider_env_vars()` injection for both local and Docker executors.

## Goals

- Tokens survive lfd restarts
- Token refresh happens without user intervention
- Container agents get credentials without host filesystem mounts
- `lfd install` guides new users through provider auth in one shot
- Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)

## Phase boundaries

- **01-token-store**: DB-backed persistence. Tokens captured at auth completion, status reads DB first, filesystem fallback preserved. **Shipped.**
- **02-proactive-refresh**: Background task refreshes tokens before expiry. Event emission for success/failure. Graceful fallback when provider CLI doesn't support refresh. **In progress.**
- **03-credential-injection**: Both local and Docker executors pull tokens from DB at agent launch. No host env dependency for API keys. **Shipped.**
- **04-install-onboarding**: `lfd install` includes interactive provider auth setup. At least one agent provider + GitHub required to complete.


## Risks

- **Provider refresh paths vary.** `gh auth refresh` exists. `claude auth refresh` may not. Must detect absent refresh commands and fall back to emitting an `auth.refresh_required` event.
- **Token format drift.** Claude's `~/.claude/.credentials.json` and Codex's `~/.codex/auth.json` are not documented APIs. Parse defensively, log warnings on unexpected formats.
- **GitHub SSH vs HTTPS.** Token injection covers API calls, but `git push` over SSH still needs host credential config.
- **Concurrent refresh races.** Two refresh attempts for the same provider could race. Background task should hold a per-provider lock during refresh.

## Metrics

- `lfq auth status` shows `active` for all connected providers after lfd restart (tokens survived in DB)
- Background refresh fires at least once when a token is within threshold of expiry
- A container-mode agent run succeeds with credentials injected from DB (no host env var set)
- `lfd install` on a clean machine prompts for provider auth and completes without errors
