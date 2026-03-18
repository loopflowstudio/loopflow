# 01: Secrets Provider

**Finish line:** Users connect a secrets provider (Doppler first) via OAuth in Concerto or CLI, and API keys for Claude/Codex populate automatically. Concerto shows which keys are available and which are missing.

## Context

API key auth exists server-side (`PUT /v0/auth/{provider}/credential`) but has no UI beyond CLI. Manual key paste doesn't scale — keys rotate, go stale, sit in plaintext. A secrets provider handles lifecycle.

The abstraction is `SecretsProvider` (Rust trait). Doppler is the first implementation. 1Password, Vault, etc. are future implementations of the same trait.

## What to build

### Rust: `SecretsProvider` trait + Doppler implementation

- `SecretsProvider` trait: `id()`, `display_name()`, `fetch_secrets(token, config) -> HashMap<String, String>`
- `DopplerSecretsProvider`: OAuth device flow, fetches via Doppler API (`GET /v3/configs/config/secrets`)
- `SecretsConfig`: project/config selection (Doppler-specific fields now, extensible for other providers)
- Secrets sync module: provider-agnostic. Fetches secrets, matches against known key mappings, stores via `configure_credential` internal path
- Known key mappings: `ANTHROPIC_API_KEY` → Claude, `OPENAI_API_KEY` → Codex

### Rust: endpoints

- `GET /v0/secrets/status` — returns connected provider, config, and which keys are present/missing
- `POST /v0/secrets/sync` — triggers manual re-sync
- `PUT /v0/secrets/config` — sets project/config for the connected provider

Doppler auth reuses the existing OAuth device flow pattern (same as GitHub/Claude/Codex).

### CLI

```bash
lf auth doppler                                    # OAuth device flow
lf auth doppler --project loopflow --config dev    # set source
lf auth doppler --sync                             # manual re-sync
lf auth doppler --disconnect                       # remove provider + keys it supplied
```

### Swift: models

- `SecretsProviderStatus`: provider id, display name, connected bool, config, supplied keys
- `SuppliedKey`: env var name, target provider, present bool

### Concerto: secrets section in Connection Settings

Below provider cards, a "Secrets" section. Provider-agnostic — shows whichever provider is connected, which keys it supplied, which are missing. "Connect Doppler" button when no provider is connected. "Refresh" to re-sync. Both platforms (shared view in LoopflowCore, platform wrappers in ConnectionSettingsView / ConnectionSetupView).

### Sync triggers

- Initial provider connect
- Concerto foreground / reconnect
- Manual "Refresh" in UI

### Disconnect behavior

Disconnecting a secrets provider clears the API keys it supplied. Clean break, no orphaned credentials.

## Constraints

- lfd owns the sync — Concerto shows status, doesn't fetch secrets directly
- Project/config is explicit (no auto-detect for now)
- No manual SecureField fallback
- GitHub is OAuth-only — no key mapping for it

## Done when

- `cargo test` passes with `SecretsProvider` trait tests and Doppler sync tests
- `swift test --package-path swift` passes with `SecretsProviderStatus` model tests
- OAuth device flow connects to Doppler and stores token
- Secrets sync populates Claude/Codex credentials from Doppler config
- Concerto shows secrets provider status and key availability on both platforms
- Disconnecting secrets provider clears supplied keys
