---
asana_id: '1213717741074343'
linear_id: 80e2a5e9-eb00-4507-b03b-0658dfabcb1a
---
# 06: Team Auth Mode

**Finish line:** `auth.mode: team` lets a self-hosted `lfd` run login, callback, token refresh, and request validation without the separate studio auth service.

## Carried context

- `auth.mode` is now the canonical config surface; `auth.provider` is rejected, and the pre-shared bearer-token mode is gone.
- Remote self-hosted deploy docs now assume the host already has valid studio credentials (`~/.lf/credentials.json`) and can register successfully. Team mode should remove that operator dependency.
- iOS no longer has a manual host/token connection screen. Remote access should continue to flow through discovery and `AuthService`, not a second manual setup path.
- `studio` mode still uses `base_url` plus connection-token distribution; `local` behavior should stay unchanged.
- Provider-auth, registration, and token-ledger plumbing already live in `lfd` and should be reused where possible.

## What to build

1. Add a `Team` auth mode with WorkOS credentials and JWT signing config.
2. Serve the OAuth routes in `lfd` (`/v0/auth/login`, `/v0/auth/callback`, `/v0/auth/refresh`) and validate issued JWTs locally.
3. Let clients choose auth server URL per connection so team mode can point both auth and wave traffic at the same `lfd`.
4. Remove the remaining dependency on the hosted studio auth service for self-hosted teams.

## Risks

- Team mode touches both daemon auth and client sign-in flows, so partial rollout can strand remote users.
- JWT issuance and refresh add new secret-management requirements for self-hosted deployments.

## Done when

- `lfd` starts in team mode with WorkOS credentials.
- OAuth works end to end: login, WorkOS callback, refresh, authenticated request.
- Swift clients connect to a team-mode server without the hosted studio auth service.
- Tests pass across Rust, Python, and Swift.
