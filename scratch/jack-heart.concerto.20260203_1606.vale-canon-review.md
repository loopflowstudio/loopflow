# Review — Concerto Auth + WaveService Refactor

## What was implemented
- Added Swift auth client primitives (AuthService/AuthState/AuthError) with Keychain storage, OAuth via ASWebAuthenticationSession, and token refresh support.
- Introduced WaveServiceProtocol + EventServiceProtocol, renamed service implementations to LocalWaveService/LocalEventService, and updated app state to use them.
- Moved Swift lfd integration to the HTTP API for waves, wave runs, and worktrees; added response parsing helpers and LFD base URL constant.
- Added URL scheme registration for loopflow:// callbacks and updated Concerto docs to describe the new service split.
- Standardized lfd HTTP port via DEFAULT_PORT (2486) and wired it through server, launchd, and CLI health checks.

## Key choices
- Use a protocol boundary for wave and event services so local/remote implementations can swap without UI changes.
- Favor HTTP API for data reads (waves, runs, worktrees) while keeping socket events for live updates.
- Centralize the HTTP port in `http_server.DEFAULT_PORT` and reuse that in clients to avoid drift.

## How it fits together
- Concerto now consumes `LocalWaveService` (HTTP) for wave state, `LocalEventService` (socket) for live events, and `WorktreeService` (HTTP first, CLI fallback) for worktree data.
- Auth primitives live in LoopflowCore to support future remote wave services while remaining unused by local-only flows today.
- lfd exposes both legacy HTTP and v1 JSON endpoints; Concerto uses `/waves`, `/wave-runs`, `/worktrees`, and `/v1/waves/{id}/connect`.

## Risks and bottlenecks
- Token refresh contract is assumed (`/auth/refresh` accepts Bearer token and returns `token`, `jwt`, or raw string). If the server differs, silent refresh will fail.
- Port change to 2486 requires all external clients/scripts to follow DEFAULT_PORT; older tools may still assume 8765.
- AuthState refresh loop runs continuously; if the refresh endpoint is flaky it may keep erroring without surfacing unless the token is nil.

## What’s not included
- RemoteWaveService implementation and wiring; TokenProvider is defined but not yet used by any remote client.
- Server-side WorkOS integration and JWT validation (handled in `roadmap/rust/05-auth.md`).
- iOS target/platform support updates (package still macOS-focused).

## Tests
- `uv run pytest tests/`
- `swift test --package-path swift` (build warnings from ghostty static lib unresolved ImGui symbols)
