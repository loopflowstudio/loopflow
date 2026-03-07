# 02: Concerto API Key Entry

**Finish line:** Users can enter, view, and switch API keys for Claude/Codex/OpenCode directly in Concerto's Connection Settings — no CLI required.

## Context

API key auth shipped with CLI-only key entry (`lfq auth configure <provider> --credential apikey`). The `AuthProviderCard` shows a credential type badge, but there's no UI for entering or switching keys. Users without terminal access can't opt into API key auth.

### From Phase 1 auth redesign (shipped)

- `FileCredentialReader` reads Claude (`~/.claude/.credentials.json`) and Codex (`~/.codex/auth.json`) credentials from disk, including nested `tokens.access_token` for ChatGPT-style Codex auth.
- `CredentialSocketServer` serves these via `/credentials/{provider}` endpoints for the containerized daemon.
- Keychain fallback exists for GitHub (`gh:github.com` service) and Claude/Codex safe storage.
- API keys stored in DB via `access_token` column, differentiated by `credential_type` — this is the same column API key entry will write to.
- The design doc proposes an `ApiKeyStep { placeholder, help_url, validate_on_submit }` model for Phase 2 typed auth methods — API key entry UI should align with that shape.

## What to build

### ConnectionSettingsView

Add an "API Keys" section below the provider cards in Connection Settings:

- For each provider that supports API keys (Claude, Codex, OpenCode):
  - If env key detected: show masked value, "Use this key" button
  - SecureField for manual entry
  - Billing model warning inline (`statusWarning` color)
  - "Switch to OAuth" nudge when using API key
- Calls `PUT /v0/auth/{provider}/credential` on save — endpoint already exists
- `AuthProviderStatus.credentialType` already in the Swift model

### Decisions from prior work

- API keys stored in DB via `access_token` column, differentiated by `credential_type`
- Connecting via OAuth auto-switches from apikey to oauth (no confirmation needed)
- Only show the active credential in status — showing both invites confusion
- Concerto stores keys in DB, not keychain (consistent with OAuth token storage)

## Done when

1. SecureField accepts API key input for Claude, Codex, OpenCode
2. Masked key display when a key is stored
3. Billing warning shown inline with `statusWarning` styling
4. "Switch to OAuth" action works from the API key view
5. Env-detected keys shown with "Use this key" option
