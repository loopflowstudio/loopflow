# Auth

## Vision

lfd becomes a real OAuth token broker. Tokens in the DB, proactive refresh, credential injection into executors, guided onboarding at install. Not multi-user token isolation, not lfd implementing OAuth PKCE directly, not token encryption beyond filesystem permissions.

## Strategy

DB-backed token management. Tokens flow into lfd's database at login. A background task refreshes them before expiry (shipped — GitHub/Codex self-heal via CLI; Claude/OpenCodeZen emit `auth.refresh_required` since they can't self-refresh). Executors pull from the DB at launch — no host mounts, no env vars assumed from the host shell. Filesystem heuristics stay as fallback for existing installs.

Remaining: API key auth as an explicit opt-in alongside OAuth, with mutual exclusivity per provider and clear billing warnings.

## Goals

- Token refresh happens without user intervention (done for GitHub/Codex; Claude/OpenCodeZen surface `auth.refresh_required`)
- Existing installs with filesystem-based auth continue working (DB is primary, filesystem is fallback)
- Users choose one credential type per provider (OAuth or API key), never both

## Risks

- **API key billing surprise.** API keys in the environment can silently bill per-token instead of using a subscription. Mutual exclusivity and billing warnings must be prominent enough to prevent this.
- **Credential type migration.** Existing installs may have both OAuth tokens and API keys for the same provider. The transition to mutual exclusivity needs a clear default (OAuth wins) and user communication.

## Metrics

- Background refresh fires at least once when a token is within threshold of expiry
- `lfq auth status` shows token expiry countdown and refresh schedule
- API key providers show billing model in `lfq auth status` and Concerto
