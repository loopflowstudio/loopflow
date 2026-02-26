# Provider auth broker review

## What was implemented
- Added provider-auth brokering in `lfd` for GitHub, Claude, and Codex (`provider_auth.rs`) with a shared `AuthBroker` trait and a `ProviderAuthService` that manages status checks, auth flow startup, pending tracking, and disconnect.
- Added authenticated HTTP endpoints:
  - `GET /v0/auth`
  - `GET /v0/auth/:provider`
  - `POST /v0/auth/:provider`
  - `DELETE /v0/auth/:provider`
- Added auth lifecycle WebSocket events (`auth.flow_started`, `auth.connected`, `auth.failed`, `auth.disconnected`).
- Wired provider auth service into daemon HTTP state and router initialization.
- Added GitHub credential mount support (`gh` / `.config/gh`) in Docker and compose credential resolution.
- Added Python client + API + CLI support for auth status/connect/disconnect (`lfq auth ...`), plus auth response models.
- Added/updated tests for provider auth parsing/status behavior, HTTP auth route helpers, Docker/compose mount resolution, Python client/models/CLI rendering.

## Key choices
- **Broker, not token store**: `lfd` delegates auth to native CLIs (`gh`, `claude`, `codex`) and derives status from CLI output/filesystem state.
- **Async flow handling**: auth start returns once a verification URL is captured; completion is tracked in a background monitor task and surfaced via events.
- **Provider-specific status/disconnect logic**:
  - GitHub: `gh auth status`/`hosts.yml`
  - Claude: auth-like files under `~/.claude`
  - Codex: `~/.codex` directory presence
- **Credential safety in containers**: GitHub mount is read-only.
- **Error surface tightening in polish pass**: provider parsing now uses a dedicated parse error type (`ParseProviderError`) instead of `()`.

## How it fits together
`POST /v0/auth/:provider` calls `ProviderAuthService::start_auth`, which launches the provider CLI auth command, parses a verification URL from output, and returns it to the caller. `ProviderAuthService` tracks pending auth flows and emits auth lifecycle events through `EventHub` as monitor tasks finish. Status/disconnect APIs call provider brokers directly, and Docker/compose mount resolution allows authenticated credentials to be available inside agent containers.

## Risks and bottlenecks
- CLI output parsing depends on current stdout/stderr formats; provider CLI output changes could break URL/code extraction.
- Claude auth/disconnect logic is filesystem-heuristic-based and may need adjustment if credential filenames/layout change.
- `gh`/`claude`/`codex` binary availability is required on host for full auth flows.
- Auth flow responsiveness is gated by URL detection timeout and polling loops; failures surface as server errors.

## What's not included
- Concerto Connections UI implementation.
- Repo onboarding (`POST /v0/repos`) and repo-first workflows.
- Additional providers (for example Gemini) beyond the three implemented.
- OAuth/token lifecycle management owned by provider CLIs (refresh/reissue behavior is not handled by `lfd`).
