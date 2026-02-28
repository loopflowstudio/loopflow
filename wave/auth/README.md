# Auth

## Vision

lfd becomes a real OAuth token broker. Tokens in the DB, proactive refresh, credential injection into executors, guided onboarding at install. Not multi-user token isolation, not lfd implementing OAuth PKCE directly, not token encryption beyond filesystem permissions.

## Strategy

DB-backed token management. Tokens flow into lfd's database at login. A background task refreshes them before expiry. Executors pull from the DB at launch — no host mounts, no env vars assumed from the host shell. Filesystem heuristics stay as fallback for existing installs.

### Invariants

- One credential type per provider (OAuth or API key), never both
- OAuth takes priority when both exist
- DB is primary, filesystem is fallback — existing installs continue working
- Never print or log API key values

## Goals

- Token lifecycle is invisible to users — refresh, rotation, and injection happen without intervention
- Users understand the cost model of their auth method before committing to it

## Risks

- API keys in the environment can silently bill per-token instead of using a subscription
- Existing installs may have both OAuth tokens and API keys for the same provider

## Metrics

- % of token expiry events that resolve via automatic refresh without user intervention (target: 100% for GitHub/Codex)
- Mean time between `auth.refresh_required` emission and user re-authentication (lower = better UX signal)
- Number of unintentional API-key billing sessions per week (target: 0 after onboarding warns)
