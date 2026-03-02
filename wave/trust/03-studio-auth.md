# 03: Studio Auth

**Finish line:** Concerto sign-in uses `auth.loopflow.studio`. lfd validates user JWTs locally via JWKS. Static token remains available as fallback.

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
- Validate claims (`iss`, `aud`, `exp`, `nbf`) and enforce `allowed_users`; reject `alg:none`
- JWKS fail-closed: reject Studio auth when keys unavailable on startup or stale on refresh; don't block startup
- Operator docs: revocation window (cache TTL + refresh cadence) and JWKS outage behavior
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

## Done when

- Studio-auth remote connection works end-to-end on both lanes
- lfd auth is local-JWKS validated and resilient to auth-server latency
- JWKS startup/refresh failures are fail-closed with explicit stale-key policy
- JWT claim checks (`iss`, `aud`, `exp`, `nbf`) and `alg:none` rejection are enforced
- Operator docs cover revocation window and JWKS outage behavior
- Concerto UX supports sign-in, refresh, and machine selection without manual host entry
- Failure modes are explicit (auth failed, token expired, no daemons, discovery unavailable)
