# 01: Provider Auth Broker

Status: **shipped** (branch `jack-heart.lfd-repos.20260225_1836`)

## What shipped

- `lfd` provider auth broker with GitHub, Claude, and Codex implementations.
- Auth HTTP routes:
  - `GET /v0/auth`
  - `GET /v0/auth/:provider`
  - `POST /v0/auth/:provider`
  - `DELETE /v0/auth/:provider`
- Auth lifecycle events: `auth.flow_started`, `auth.connected`, `auth.failed`, `auth.disconnected`.
- Docker/compose credential mount expansion for GitHub (`gh` / `.config/gh`) as read-only.
- Python API/client/models and `lfq auth ...` CLI commands.
- Tests for provider parsing/status behavior, routes, mount resolution, and Python auth surfaces.

## Contract now

`lfd` is an auth broker, not a token store:

1. Launch provider CLI login flow.
2. Capture verification URL/device code.
3. Return URL to caller and emit auth lifecycle events.
4. Derive status from provider CLI/filesystem state.
5. Disconnect via provider logout or credential cleanup.

## Follow-ups promoted from scratch

- Validate `GH_BROWSER=echo gh auth login --web ...` behavior against live GitHub CLI runs.
- Validate `claude login` URL capture and update parsing/flags if output differs.
- Verify Claude credential file matching for disconnect logic beyond current filename heuristics.
