# Gate review — remote EC2 dogfood deploy + smoke harness

## What was implemented

- Added production-facing remote deploy docs at `deploy/README.md` for EC2 + Docker + Caddy TLS + static token auth.
- Added `scripts/test_remote_smoke.py`, a remote smoke harness that verifies:
  - `/health`
  - wave CRUD
  - auth rejection
  - SSE session events
  - websocket `connected` handshake
  - wave run + log streaming
  - websocket reconnect snapshot behavior
- Extended shared HTTP harness (`scripts/lib/api_harness.py`) with TLS verify controls used by remote smoke scripts.
- Updated deployment config:
  - `deploy/Caddyfile` now supports env-driven domain/TLS mode, websocket routing, and SSE-friendly `flush_interval -1`.
  - `deploy/docker-compose.prod.yml` now forwards `LF_DOMAIN`, `LF_TLS_MODE`, and exposes port 80 for ACME.
- Added `websockets` to dev dependencies and lockfile.
- Updated `TESTING.md` with remote smoke script usage.

## Key choices

- **One script validates the full remote path** (auth + HTTP + SSE + WS + run logs) instead of documenting manual checks only.
- **TLS flexibility in script**: supports default verification, custom CA trust (`--ca-cert`), and explicit insecure mode for internal CA bootstrapping.
- **Repo selection fallback with guardrails**:
  - `--repo` override is preferred.
  - Fallback to `/v0/repos` only when available.
  - Clear error for fresh hosts with no repos (`pass --repo ...`) to avoid confusing first-run failures.
- **Session cleanup in SSE scenario**: explicit best-effort DELETE so smoke runs do not leave stray sessions.

## How it fits together

`deploy/docker-compose.prod.yml` brings up `lfd` behind Caddy, with `deploy/Caddyfile` handling TLS termination plus proxy behavior needed for SSE/WS. `scripts/test_remote_smoke.py` then exercises the same remote endpoint Concerto uses, validating transport/auth/session/wave behaviors over the TLS proxy. `deploy/README.md` ties infra setup and smoke verification into a single operator path.

## Risks and bottlenecks

- Smoke still depends on a valid remote repo path for session/wave scenarios; fresh hosts must provide `--repo`.
- Session scenario depends on the requested harness (`--session-harness`, default `claude`) being configured on the remote host.
- Wave/log scenarios remain timing-sensitive on very slow hosts (`--events-timeout`, `--logs-timeout` may need tuning).
- Manual Concerto-specific checks (remote editor/terminal UX) are still operator-driven, not automated by this script.

## What's not included

- No host provisioning automation (Terraform/cloud-init) in this branch.
- No studio/JWT auth rollout work.
- No remote repo discovery API changes; script uses existing `/v0/repos` behavior.
- No CI integration for remote smoke against external hosts; this remains an operator-run validation command.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `uv run pytest python/tests/ -q`
- `uv run pytest tests/e2e/test_api_smoke.py -q`
- `uv run python scripts/test_remote_smoke.py --help`
