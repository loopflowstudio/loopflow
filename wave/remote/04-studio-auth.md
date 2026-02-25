# 04: Studio Auth

Replace static-token remote auth with studio identity, after dogfood and fork cleanup are stable.

## What exists after this

- Concerto sign-in uses `auth.loopflow.studio`
- lfd validates user JWTs locally via JWKS (no per-request auth roundtrip)
- Daemon discovery lets users choose their machine after sign-in
- Static token remains available as explicit fallback during rollout

## Cross-repo ownership

This phase spans two repos:

- `loopflow.remote`: lfd JWT validation path, Concerto token wiring, sign-in/discovery UX
- `../studio`: auth endpoints, JWKS publishing, daemon register/discover APIs, infra config

Ship this phase only when both repos pass the same end-to-end flow.

## Scope

### In scope (`loopflow.remote`)

- Replace studio `ConnectionValidator` path with cached JWKS validation in lfd
- Validate claims (`iss`, `aud`, `exp`) and enforce `allowed_users`
- Wire `TokenProvider` into API/event requests in Concerto
- Add sign-in + daemon picker flow in connection UX
- Keep static-token mode available as fallback

### In scope (`../studio`)

- Keep auth + device flow endpoints stable for clients
- Keep daemon register/heartbeat/discover contracts stable
- Ensure JWKS rotation/refresh behavior is documented and safe

### Out of scope

- Broad API expansion beyond auth/discovery needs
- Hosted multi-tenant control plane

## Integration contract

- JWT issuer: `auth.loopflow.studio`
- JWT audience: `loopflow-lfd`
- JWKS endpoint: `/.well-known/jwks.json`
- Discovery endpoint returns machine metadata needed for picker UX

Any contract change requires same-iteration updates in both repos.

## Validation

From laptop, verify both EC2 and Mac Mini lanes:

1. Sign in from Concerto
2. Discover daemon(s)
3. Connect and run a wave end-to-end
4. Stream events/logs over remote connection
5. Expire/refresh token and verify reconnect path
6. Confirm static-token fallback still works when selected

## Done when

- Studio-auth remote connection works end-to-end on both lanes
- lfd auth is local-JWKS validated and resilient to auth-server latency
- Concerto UX supports sign-in, refresh, and machine selection without manual host entry
- Failure modes are explicit (auth failed, token expired, no daemons, discovery unavailable)
