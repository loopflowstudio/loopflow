# lfd-repos connections panel review

## What was implemented

- Added shared provider-auth models in `LoopflowCore` (`AuthProvider`, `AuthProviderStatus`, `AuthFlow`, wrapped list response).
- Added `/v0/auth` service methods to `LocalWaveService` for list/get/start/disconnect flows.
- Extended websocket parsing in `LocalEventService` for `auth.flow_started`, `auth.connected`, `auth.failed`, and `auth.disconnected`.
- Added `AuthProviderStore` to own provider auth state, pending flows, browser launch requests, 409 reconciliation, and reconnect refresh.
- Wired `RepoState` to bind/rebind auth services, forward auth events, and refresh auth state on connection transitions.
- Added shared `AuthProviderCard` UI and integrated provider connections into macOS `ConnectionSettingsView` and iOS `ConnectionSetupView`.
- Added tests for model decoding, auth event parsing, auth HTTP methods, and auth store transitions.
- Polish pass: fixed provider error visibility in `AuthProviderCard` so disconnect/connect errors are shown for any card state, and added coverage for disconnect-failure behavior.

## Key choices

- Kept auth state in a dedicated `AuthProviderStore` instead of mixing with daemon connection state.
- Modeled pending auth per provider (`pendingFlows` map), not globally.
- Kept browser launch side effects in platform views; store emits `browserLaunchRequest` only.
- Treated HTTP `409` from auth start as continuity (refresh + preserve pending), not terminal failure.
- Used stable provider ordering via `AuthProvider.allCases` for deterministic UI/tests.

## How it fits together

- HTTP (`/v0/auth`) provides authoritative snapshots, websocket `auth.*` events provide incremental updates, and `AuthProviderStore` reconciles both streams into a single observable state.
- `RepoState` is the integration point: it forwards connection and auth events into `AuthProviderStore`, while iOS/macOS views render `AuthProviderCard` from store state and perform browser launch/copy interactions.

## Risks and bottlenecks

- Multi-client races remain possible (event ordering vs refresh timing), though refresh-on-reconnect and 409 handling reduce drift.
- Browser launch depends on platform APIs and user settings; fallback UX is in place but still user-dependent.
- Full macOS Xcode scheme (`Concerto`, including UI tests) currently fails in this environment due `ConcertoUITests-Runner` early bootstrap exit before test connection; non-UI tests pass.

## What's not included

- No server contract changes.
- No repo onboarding auth integration (`POST /v0/repos`) in this step.
- No token management/inspection UI.
- No additional providers beyond GitHub/Claude/Codex.

## Validation run

- `cargo fmt --all -- --check` ✅
- `cargo clippy --all-targets -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `swift test --package-path swift` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py -v` ✅
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ fails from `ConcertoUITests-Runner` bootstrap crash in this host environment.
