# Review: Concerto file-based remote connection seeding

## What was implemented
- Added `LoopflowCore/Config/ConcertoConfig.swift` with `ConcertoConfig`, `RemoteConnectionConfig`, and `loadConcertoConfig()` to read `~/.lf/concerto.yaml`.
- Added conversion from config connection to `ServerConnection` with enforced TLS + static-token auth.
- Added loopback-host guard (`localhost`, `127.0.0.1`, `::1`) so local dev still uses bundled daemon.
- Updated `ConnectionStore` startup path to load in this priority order:
  1. persisted UserDefaults settings,
  2. seeded config from `~/.lf/concerto.yaml`,
  3. bundled default fallback.
- Seeded config state is persisted on first launch so subsequent launches use UserDefaults (YAML only seeds first launch).
- Added tests for config loading and startup priority/seed behavior, including loopback rejection and nested-key ignore behavior.

## Key choices
- **Seed-once model:** YAML is only a bootstrap source; persisted settings win after first launch. This preserves user override behavior.
- **No token in YAML:** token continues to come from `ConnectionSecretStore`/Keychain via existing `<host>:<port>` lookup.
- **Top-level-only `connection` parsing:** nested `connection` keys are ignored to avoid accidental seeding from unrelated YAML sections.
- **Loopback rejection at seed boundary:** config can exist but will not switch to remote mode for loopback hosts.

## How it fits together
`ConnectionStore` now receives a config loader during initialization and consults it only when no persisted state exists. `ConcertoConfig` parsing maps YAML host/port to a `ServerConnection` in remote mode with fixed TLS/static-token semantics, then `ConnectionStore` resolves token material from Keychain and persists resulting settings to UserDefaults.

## Risks and bottlenecks
- YAML parsing is intentionally narrow (line-based for the expected schema). It is robust for the expected format but not a full YAML implementation.
- Invalid/non-integer ports or malformed sections fail closed to bundled mode (safe behavior, but can hide config mistakes unless inspected).
- If Keychain token is missing for seeded remote connection, startup still selects remote mode; connection attempts will then fail until token is added.

## What's not included
- No installer/studio changes in this repo (studio writes YAML + Keychain token in its own repo).
- No changes to `ConnectionSecretStore`, `ServerConnection`, Rust, or Python code.
- No support for non-TLS remote config fields (`tls`, inline token) by design.

## Validation
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
