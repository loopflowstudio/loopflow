# Auth

## Vision

lfd becomes a real OAuth token broker. Tokens in the DB, proactive refresh, credential injection into executors, guided onboarding at install. Not multi-user token isolation, not lfd implementing OAuth PKCE directly, not token encryption beyond filesystem permissions.

## Strategy

DB-backed token management. Tokens flow into lfd's database at login. A background task refreshes them before expiry. Executors pull from the DB at launch — no host mounts, no env vars assumed from the host shell. Filesystem heuristics stay as fallback for existing installs.

## Goals

- Token refresh happens without user intervention
- Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)

## Risks

- **Provider refresh paths vary.** `gh auth refresh` exists. `claude auth refresh` may not. Must detect absent refresh commands and fall back to emitting an `auth.refresh_required` event.
- **Concurrent refresh races.** Two refresh attempts for the same provider could race. Background task should hold a per-provider lock during refresh.

## Metrics

- Background refresh fires at least once when a token is within threshold of expiry
- `lfq auth status` shows token expiry countdown and refresh schedule
