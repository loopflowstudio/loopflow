# Remote EC2 dogfood deploy + smoke harness

## Scope

This branch adds a production-oriented remote deployment path for `lfd` on EC2 and a single smoke command to validate the full remote API path over Caddy/TLS.

## Current state

### Deployment docs and config

- `deploy/README.md` documents EC2 setup, Docker/Caddy deployment, auth token handling, and smoke verification.
- `deploy/Caddyfile` now supports env-driven domain/TLS mode, websocket routing, and SSE-friendly `flush_interval -1`.
- `deploy/docker-compose.prod.yml` now forwards `LF_DOMAIN` and `LF_TLS_MODE` and exposes port 80 for ACME HTTP-01.

### Remote smoke coverage

- `scripts/test_remote_smoke.py` validates:
  - `/health`
  - auth rejection behavior
  - wave CRUD
  - SSE session events
  - websocket `connected` handshake
  - wave run + log streaming
  - websocket reconnect snapshot behavior
- `scripts/lib/api_harness.py` now supports TLS verification controls needed for remote smoke.
- `python/tests/test_remote_smoke_script.py` covers script behavior.
- `TESTING.md` includes remote smoke usage.

## Key decisions

- Use one end-to-end remote smoke script instead of fragmented manual checks.
- Support three TLS modes in smoke: default verification, custom CA (`--ca-cert`), and explicit insecure mode (`--insecure`) for internal CA bootstrap.
- Prefer explicit `--repo`; fallback to `/v0/repos` only when available, with a clear error when none exist.
- Best-effort cleanup of created session resources after SSE checks.

## Constraints and risks

- Fresh hosts still require an explicit `--repo` for session/wave scenarios.
- Session checks require a configured harness (`--session-harness`, default `claude`) on the remote host.
- Slow hosts may need higher `--events-timeout` and `--logs-timeout`.
- Concerto UX checks remain manual (remote editor/terminal flows are not automated here).

## Explicitly out of scope

- Host provisioning automation (Terraform/cloud-init)
- Studio/JWT auth rollout
- `/v0/repos` API changes
- CI automation against external remote hosts

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `uv run pytest python/tests/ -q`
- `uv run pytest tests/e2e/test_api_smoke.py -q`
- `uv run python scripts/test_remote_smoke.py --help`
