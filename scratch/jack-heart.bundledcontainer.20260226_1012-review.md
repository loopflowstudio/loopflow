# Bundled container review

## What was implemented

- Replaced Concerto's bundled daemon launch path from a native `Process` fork to Docker container orchestration in `BundledDaemonManager`.
- Added a macOS credential Unix-socket server (`CredentialSocketServer`) that proxies provider credentials from Keychain (`github`, `claude`, `codex`) to containerized `lfd`.
- Added Rust credential-socket client support and socket-backed auth broker (`SocketAuthBroker`) so `lfd` can use Keychain-backed credentials when `LFD_CREDENTIAL_SOCKET` is set, with CLI broker fallback otherwise.
- Added credential socket config plumbing (`credential_socket`) and container env injection for agent credentials (`GH_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`).
- Added Concerto config support for `container.image` and `container.mounts` plus parsing/tests for `~/.lf/concerto.yaml` mount syntax.
- Added Connection Settings UI treatment for Docker-unavailable fallback (native-mode button + Docker Desktop link).
- Polish pass fix: socket auth monitoring now waits/polls for credentials (instead of resolving immediately), and includes a new test `socket_auth_monitor_waits_until_credential_exists`.

## Key choices

- **Credential transport over Unix socket, not direct Keychain access in containers**: keeps Keychain trust anchored in the host app while still enabling token-based auth inside Docker.
- **Dual auth broker model**: `ProviderAuthService::new()` selects socket brokers only when `LFD_CREDENTIAL_SOCKET` is available, preserving compatibility with non-Concerto and CLI-only flows.
- **Container launch keeps existing ephemeral auth model**: Concerto still generates runtime token+port per launch; only daemon transport changed.
- **Global extra mounts via config**: simple, explicit host-path passthrough under `/workspace/extra/*`, no per-agent mount policy in this iteration.

## How it fits together

Concerto starts `CredentialSocketServer`, then runs `loopflow/lfd` in Docker with `/workspace/src` (ro), Docker socket, credential socket, and runtime env (`LFD_CREDENTIAL_SOCKET`, auth token, port). Inside `lfd`, provider auth and executor credential loading use the socket client to fetch provider tokens; executor injects them as env vars for agent containers. Agent containers remain repo-scoped (one repo rw mount) while inheriting only configured credentials/mounts.

## Risks and bottlenecks

- **Auth start flow is still lightweight**: `POST /auth/{provider}/start` opens provider URL and returns metadata, but does not run a full in-app OAuth state machine yet.
- **Docker dependency UX**: users without Docker Desktop will hit startup failure and must use native fallback.
- **Startup latency**: `docker pull` during container start can add delay depending on network/image state.
- **UITest stability**: local `xcodebuild test -scheme Concerto` runs currently fail at UITest runner bootstrap (signal kill), though Swift package tests and non-UI suites pass.

## What's not included

- Full provider-specific OAuth/device-code orchestration inside Concerto.
- Per-agent custom mount policies or finer-grained mount scoping beyond global `container.mounts`.
- Any change to remote-mode connection behavior beyond config seeding and connection-settings UX.
