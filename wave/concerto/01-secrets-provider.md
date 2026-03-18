# 01: Secrets Provider

**Finish line:** Users connect a secrets provider (Doppler first) from Concerto or CLI, and Claude/Codex API keys populate automatically. Concerto shows which expected keys are present and which are missing.

## Context

API key auth already exists server-side via `PUT /v0/auth/{provider}/credential`, and the existing provider cards already surface `credential_type == apikey`. What does not exist is a durable way to source and refresh those keys without pasting them by hand.

The current Swift auth model only exposes GitHub, Claude, and Codex. For this first pass, the secrets-provider mapping only needs to light up Claude and Codex. Concerto already has connection surfaces on macOS (`ConnectionSettingsView`) and iOS (`ConnectionSetupView`); the missing piece is provider-agnostic secrets status below those cards. `AuthProvider` and `AuthProviderCard` stay harness-provider-specific, so Doppler status needs its own shared model and view section instead of pretending to be another auth provider.

## What to build

### lfd auth and sync

1. Add a provider-agnostic `SecretsProvider` boundary in lfd plus a `DopplerSecretsProvider` implementation.
2. Store the secrets-provider token/config separately from harness provider tokens.
3. After connect or refresh, fetch secrets, match known env vars, and persist matching keys through the existing credential-storage path.
4. On disconnect, clear the keys supplied by that secrets provider.

### API and CLI

1. Add secrets endpoints for status, sync, and project/config updates.
2. Add `lf auth doppler` commands for connect, project/config selection, sync, and disconnect.
3. Keep project/config explicit for the first pass; no auto-detect heuristics yet.

### Swift models and views

1. Add `SecretsProviderStatus` and `SuppliedKey` models in shared Swift code.
2. Add a provider-agnostic "Secrets" section below the existing provider cards on both platforms.
3. Show connected provider, current project/config, present keys, missing keys, refresh, and disconnect.
4. Reuse the existing provider cards for the final credential state; don't build a second auth status surface.

## Constraints

- lfd owns secrets fetching and key sync; Concerto only displays status and triggers actions
- No manual SecureField fallback in this item
- Claude and Codex are the only key mappings in scope for this first pass
- Keep the abstraction cheap: adding 1Password or Vault later should be a new implementation, not a rewrite

## What this item should teach us

- Whether secrets providers belong inside the existing provider-auth registry or beside it
- How much project/config selection UI Doppler needs before the flow feels reliable
- Whether reconnect-time sync is enough, or if later providers need stronger refresh semantics

## Done when

- `cargo test` passes with secrets-provider trait/sync coverage
- `swift test --package-path swift` passes with `SecretsProviderStatus` decoding tests
- Doppler device-flow auth stores a reusable provider token
- Secrets sync populates Claude/Codex credentials from the chosen Doppler config
- Concerto shows secrets-provider status and missing/present keys on both platforms
- Disconnecting the secrets provider removes the keys it supplied
