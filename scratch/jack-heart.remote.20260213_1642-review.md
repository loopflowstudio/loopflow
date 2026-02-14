# Remote: Compose Stack, Pre-shared Auth, EC2 Deployment

Design review for `jack-heart.remote.20260213_1642`. Phases 02, 03, and 04 (deployment artifacts) of the remote roadmap.

## What was implemented

**Phase 03: Pre-shared Token Auth.** Replaced `AuthContext` (two booleans encoding four states) with `AuthProvider` enum (`Local`, `Static`, `Studio`). Static token auth validates a pre-shared bearer token via constant-time comparison (`subtle::ConstantTimeEq`). Python client gains `token=` kwarg and `LFD_TOKEN` env var.

**Phase 02: Docker Compose Stack.** `docker-compose.yml` packages lfd + postgres into a single deployment artifact. Multi-stage Dockerfile builds minimal Debian image with `lfd` and `lf` binaries. Postgres auto-migrates on startup. Docker socket mounted for agent container creation.

**Phase 04: EC2 Deployment Artifacts.** Caddyfile for TLS termination (`tls internal`), `docker-compose.prod.yml` override adding Caddy and forcing `auth.provider=static`, `deploy.sh` for pushing code to a remote host, `.env.example` documenting required env vars. Terraform lives in the studio repo (out of scope here).

**Cleanup.** Removed unused `builtin_ops_prompt_names()`. Moved completed roadmap phases (02, 03, 04) from `roadmap/remote/` to `scratch/`. Updated roadmap README to reflect shipped status.

## Key decisions

**`AuthProvider` enum over `AuthContext` booleans.** The old struct had `active` and `registered` booleans encoding a state machine with invalid states. The enum makes each variant explicit and `#[non_exhaustive]` for future extensibility.

**Constant-time token comparison.** `subtle::ConstantTimeEq` for the static token. Zero cost, prevents timing side channels when Studio auth (Phase 07) ships.

**Loopback bypass is unconditional.** All providers skip auth for 127.0.0.1 connections. Local dev never broken by auth config.

**Non-loopback + `provider: local` warns, doesn't crash.** Remote connections get 403 but the server stays up.

**Loopback bind skips auth setup entirely.** In `lfd.rs`, when the bind address is loopback, `setup_auth()` is not called — the server uses `AuthProvider::Local` directly. Auth is only configured for non-loopback binds. This avoids requiring tokens or studio credentials for local dev.

**Run as root in container.** The Docker socket grants root-equivalent access regardless of UID. Non-root user with Docker socket access would be complexity without security benefit. Agent containers (Phase 01) are the real security boundary.

**Docker-out-of-Docker.** Mount host Docker socket. Agent containers are siblings, not nested. DinD adds complexity (privileged mode, storage drivers) for no benefit.

**Auto-migrate on startup.** When `LFD_STORAGE=postgres`, lfd runs migrations before serving. Idempotent — no-op if schema is current. Eliminates separate `lfd migrate` step.

**Compose override for prod, not a separate file.** `docker-compose.prod.yml` adds Caddy and sets `LFD_AUTH_PROVIDER=static`. The base `docker-compose.yml` works standalone for local dev. The host port mapping on lfd is kept in prod (harmless behind security group) rather than using `!reset` — simpler, less compose magic.

**Caddy with `tls internal` for self-signed certs.** No domain needed. Caddy generates a local CA and cert on first start. Concerto will pin the cert fingerprint on first connect (TOFU). Good enough for a single-user dev box.

**Deploy via git pull, not scp.** `deploy.sh` does `git pull` on the remote host then `docker compose up --build`. Only `.env` is copied via scp (secrets stay out of git). Simpler than copying individual files, and the remote always matches the repo.

**Build on the instance.** The Dockerfile uses multi-arch base images (`rust:1.82-bookworm`, `debian:bookworm-slim`). Building on ARM EC2 produces native images without cross-compilation.

## How it fits together

```
lfd.yaml / env vars
    -> LfdConfig.load() reads file + env overrides
    -> setup_auth() dispatches on provider string -> AuthProvider enum
    -> HttpState stores AuthProvider
    -> auth_middleware matches on variant per request
    -> loopback always bypasses

docker-compose.yml (local)
    -> postgres starts, healthcheck passes
    -> lfd starts, auto-migrates postgres schema
    -> lfd binds 0.0.0.0:2486 (compose env), Docker socket mounted
    -> agent containers created as siblings via Docker API

docker-compose.yml + docker-compose.prod.yml (remote)
    -> same as above, plus:
    -> Caddy starts after lfd healthcheck passes
    -> Caddy terminates TLS on :443, proxies to lfd:2486
    -> LFD_AUTH_PROVIDER=static forces token auth
```

## Risks

**Token in plaintext config.** Static token stored in `LFD_AUTH_TOKEN` env var (in `.env` file, gitignored). Acceptable for dev; security group restricts network access. Phase 07 replaces with JWT.

**Cargo build time in Docker.** Clean builds take 5-10 minutes on t4g.medium. No dependency caching (`cargo-chef`) yet. Incremental builds use Docker layer cache.

**`process::exit(1)` on config errors.** Used in `setup_auth()` for missing token, unknown provider, registration failure. Consistent with other startup-fatal paths. These only fire during startup.

**Postgres data loss on `docker compose down -v`.** Named volumes persist across `down` but destroyed by `down -v`. Dev-appropriate.

## Not included

- Terraform (lives in studio repo)
- Concerto token support (Phase 05)
- Token rotation, expiry, revocation
- JWT/JWKS validation for Studio provider (Phase 07)
- CI/CD pipeline for deploys
- `cargo-chef` dependency caching in Dockerfile
- Multi-arch Docker image registry
- Monitoring/alerting
- Domain name / Let's Encrypt
- Rate limiting on auth failures
- Middleware-level integration tests (core logic unit-tested; middleware dispatch is a straightforward match)
