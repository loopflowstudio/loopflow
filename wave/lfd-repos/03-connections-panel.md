# 03: Concerto Connections Panel

Status: **shipped** (branch `jack-heart.lfd-repos.20260225_2241`)

## What shipped

- Added shared provider-auth models in `LoopflowCore`:
  - `AuthProvider`
  - `ProviderAuthStatus`
  - `AuthProviderStatus`
  - `AuthFlow`
  - `AuthProviderListResponse`
- Added `/v0/auth` service methods to `LocalWaveService`:
  - `listAuthProviders()`
  - `getAuthProvider(provider:)`
  - `startAuthFlow(provider:)`
  - `disconnectProvider(provider:)`
- Extended websocket parsing in `LocalEventService` for:
  - `auth.flow_started`
  - `auth.connected`
  - `auth.failed`
  - `auth.disconnected`
- Added `AuthProviderStore` for provider state, pending flows, browser-launch requests, auth-event reconciliation, `409` continuity handling, and refresh-on-reconnect.
- Wired `RepoState` to rebind auth services and forward both auth events and connection-state transitions into `AuthProviderStore`.
- Added shared `AuthProviderCard` UI and integrated provider connection controls into:
  - macOS `ConnectionSettingsView`
  - iOS `ConnectionSetupView`
- Added Swift test coverage for:
  - auth model decoding,
  - auth event parsing,
  - auth HTTP methods,
  - auth store transitions.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

## Follow-ups

- Proceed with step 4: wire provider auth state into repo onboarding (`POST /v0/repos`) so the primary workflow is repo-first inside Concerto.
- Keep running full `Concerto` scheme tests in environments where `ConcertoUITests-Runner` can attach cleanly.
