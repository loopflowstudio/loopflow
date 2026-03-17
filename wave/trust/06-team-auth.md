# 06: Team Auth Mode

**Status:** Backlog. Design doc complete at `scratch/clear-the-deck-auth-consolidation.md`.

## Problem

Team deployments currently require the studio auth service for remote client access — a separate deployment with registration heartbeats and connection token distribution. Teams should be able to run lfd self-contained with their own identity provider.

## Approach

Add a `Team` variant to `AuthProvider`. lfd runs WorkOS authorization code flow directly, issues short-lived JWTs (HMAC-SHA256, 1 hour), validates them locally. Teams bring their own WorkOS credentials (`WORKOS_CLIENT_ID`, `WORKOS_API_KEY`).

New routes: `/v0/auth/login`, `/v0/auth/callback`, `/v0/auth/refresh`.

Client model: every client needs an auth server URL and a wave server URL. In team mode, both point at the same lfd. In studio mode, they're different (auth at `auth.loopflow.studio`, waves at localhost).

## Scope

- Add `Team` variant to `AuthProvider`
- WorkOS OAuth routes in lfd HTTP server
- JWT signing/validation
- Config: `auth.mode: team` with `workos_client_id`, `workos_api_key`, `jwt_secret`
- Swift AuthService: configurable auth server URL
- Rename `auth.provider` → `auth.mode`, `static` → `ci` (bundled with this or separate)

## Done when

- lfd starts in team mode with WorkOS credentials
- OAuth flow works end-to-end: login → WorkOS → callback → JWT
- JWT-authenticated requests pass middleware
- Swift AuthService connects to lfd in team mode
- Tests pass across Rust, Python, Swift
