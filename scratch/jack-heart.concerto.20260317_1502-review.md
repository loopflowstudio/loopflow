# Secrets Provider — Design Review

## What was implemented

Doppler secrets provider integration across lfd, HTTP API, and Concerto (macOS + iOS). Users connect a Doppler project, and Claude/Codex API keys populate automatically. Concerto shows which keys are present and which are missing.

### Rust (lfd)

- `SecretsProvider` async trait with `DopplerSecretsProvider` implementation — fetches from `api.doppler.com/v3` with bearer auth
- `sync_secrets()` matches fetched env vars against `KEY_MAPPINGS` (ANTHROPIC_API_KEY → Claude, OPENAI_API_KEY → Codex), stores as `ProviderToken` entries with `login: "via {provider}"`
- `clear_secrets_credentials()` only removes tokens with "via " prefix — manually-entered keys are untouched
- `secrets_status()` builds status from stored config + token presence
- `SecretsProviderConfig` stored encrypted in SQLite (migration 030)
- HTTP routes: GET/DELETE `/secrets`, POST `/secrets/connect`, POST `/secrets/sync`, PUT `/secrets/config`
- Three event types: `secrets.connected`, `secrets.synced`, `secrets.disconnected`

### Swift (Concerto)

- `SuppliedKey` and `SecretsProviderStatus` models with `CodingKeys` for snake_case bridging
- `SecretsProviderStore` — `@Observable` state container with protocol-based service injection
- `SecretsProviderSection` — shared SwiftUI view for both platforms (connect form + connected status)
- `SecretsEvent` handling through existing WebSocket event stream
- Integrated into `ConnectionSettingsView` (macOS) and `ConnectionSetupView` (iOS)

## Key choices

1. **"via " prefix as ownership marker** — credentials supplied by secrets providers are tagged with `login: "via doppler"`. Disconnect only clears tokens with this prefix. Simple, no extra schema needed.

2. **Single-provider-at-a-time model** — `list_secrets_provider_configs` returns a vec but `secrets_status` takes only the first. Adequate for first pass (Doppler only), extensible later.

3. **Protocol-based service injection in Swift** — `SecretsProviderService` protocol extended by `WaveService`. Store binds to the service at connection time. Clean testing boundary.

4. **Shared view, not platform-specific** — `SecretsProviderSection` lives in `LoopflowCore/Views/`, used by both macOS and iOS connection screens with no `#if` guards.

## How it fits together

```
Concerto → HTTP API → lfd secrets module → Doppler API
                ↓
         SQLite (encrypted config)
                ↓
         ProviderToken store (existing credential path)
                ↓
         WebSocket events → Concerto UI refresh
```

lfd owns all secrets logic. Concerto is a status display and trigger surface. The secrets module reuses the existing `ProviderToken` storage and `EventHub` broadcast — no new infrastructure.

## Risks and bottlenecks

- **Token expiry** — Doppler service tokens don't expire, but if the user rotates them in Doppler, sync will fail with 401. The error surfaces in the UI ("secrets provider token is invalid or expired") but there's no proactive refresh.
- **No auto-sync** — secrets are fetched on connect, manual refresh, or config update. No periodic polling. Fine for first pass but means stale keys if Doppler values change.
- **Postgres stub** — `SecretsProviderStore` for Postgres returns `Ok(())` / empty vec. Works for now (lfd only uses SQLite locally), but needs real implementation if remote lfd instances use Postgres.

## What's not included

- CLI commands (`lf auth doppler`) — design doc mentions these but they're not in this branch
- Device-flow auth for Doppler — this branch uses service tokens (paste a token), not OAuth device flow
- 1Password / Vault providers — by design, just Doppler for now
- Auto-detect of project/config — explicit entry only
