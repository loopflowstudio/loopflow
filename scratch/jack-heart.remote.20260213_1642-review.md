# Remote: Compose Stack + Pre-shared Auth

Design review for `jack-heart.remote.20260213_1642`. Phases 02 and 03 of the remote roadmap.

## What was implemented

**Phase 03: Pre-shared Token Auth.** Replaced `AuthContext` (two booleans encoding four states) with `AuthProvider` enum (`Local`, `Static`, `Studio`). Static token auth validates a pre-shared bearer token via constant-time comparison. Python client gains `token=` kwarg and `LFD_TOKEN` env var.

**Phase 02: Docker Compose Stack.** `docker-compose.yml` packages lfd + postgres into a single deployment artifact. Multi-stage Dockerfile builds minimal Debian image with `lfd` and `lf` binaries. Postgres auto-migrates on startup. Docker socket mounted for agent container creation.

**Cleanup.** Removed unused `builtin_ops_prompt_names()`. Moved completed roadmap phases from `roadmap/remote/` to `scratch/`.

## Key choices

**`AuthProvider` enum over `AuthContext` booleans.** The old struct had `active` and `registered` booleans encoding a state machine with invalid states. The enum makes each variant explicit and `#[non_exhaustive]` for future extensibility.

**Constant-time token comparison.** `subtle::ConstantTimeEq` for the static token. Zero cost, prevents timing side channels when Studio auth (Phase 07) ships.

**Loopback bypass is unconditional.** All providers skip auth for 127.0.0.1 connections. Local dev never broken by auth config.

**Run as root in container.** The Docker socket grants root-equivalent access regardless of UID. Non-root user with Docker socket access would be complexity without security benefit. Agent containers (Phase 01) are the real security boundary.

**Auto-migrate on startup.** When `LFD_STORAGE=postgres`, lfd runs migrations before serving. Idempotent — no-op if schema is current. Eliminates separate `lfd migrate` step for compose deployments.

**No `repos:` volume in compose.** DockerExecutor creates per-repo volumes dynamically via the Docker API. A shared compose volume would conflict.

## How it fits together

```
lfd.yaml / env vars
    -> LfdConfig.load() reads file + env overrides
    -> setup_auth() dispatches on provider string -> AuthProvider enum
    -> HttpState stores AuthProvider
    -> auth_middleware matches on variant per request
    -> loopback always bypasses

docker-compose.yml
    -> postgres starts, healthcheck passes
    -> lfd starts, auto-migrates postgres schema
    -> lfd binds 0.0.0.0:2486 (compose env), Docker socket mounted
    -> agent containers created as siblings via Docker API
```

## Risks and bottlenecks

**Token in plaintext config.** Static token stored in `lfd.yaml` or `LFD_AUTH_TOKEN` env. Acceptable for dev; Phase 04 restricts access via security groups. Phase 07 replaces with JWT.

**Cargo build time in Docker.** Clean builds take 5-10 minutes. No dependency caching (`cargo-chef`) yet. Incremental builds use Docker layer cache.

**`process::exit(1)` on config errors.** Used in `setup_auth()` for missing token, unknown provider, registration failure. Consistent with other startup-fatal paths in the codebase (`lf-agent.rs`). These only fire during startup, before the server serves any requests.

**Postgres data loss on `docker compose down -v`.** Named volumes persist across `down` but destroyed by `down -v`. Dev-appropriate; noted in `.env.example` comments.

## What's not included

- TLS termination (Phase 04 adds Caddy)
- Concerto token support (Phase 05)
- Token rotation, expiry, revocation
- JWT/JWKS validation for Studio provider (Phase 07)
- CI image builds (build locally for now)
- Multi-arch Docker images
- Rate limiting on auth failures
- Middleware-level integration tests (core logic unit-tested; middleware dispatch is a straightforward match)
