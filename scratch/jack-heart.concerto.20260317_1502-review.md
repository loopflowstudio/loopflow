# Review: Connections Panel Redesign + Secrets Provider

## What was implemented

Two integrated features shipped as one change:

**Secrets provider (Doppler)** — full Rust backend and Swift frontend for Doppler integration. Authenticate via `doppler` CLI (auto-detected), browse projects/configs via Doppler API, sync secrets into provider tokens (ANTHROPIC_API_KEY → Claude, OPENAI_API_KEY → Codex). Smart config defaults (dev > prd > prod). Disconnect removes only secrets-supplied keys.

**Connections panel redesign** — providers grouped by role (Agents, Source Control, Project Management, Secrets) instead of a flat list. `ProviderRow` replaces `AuthProviderCard`. `ProviderGroupSection` groups rows under a role header. Secrets group has a footer that expands inline to show project/config pickers and key status when Doppler is connected.

## Key choices

1. **Removed `SecretsProvider` trait** — only one implementation (Doppler), so the trait was premature abstraction. Direct functions (`list_projects`, `list_configs`, `fetch_secrets`, `sync_secrets`) are simpler and testable.

2. **Token stored in `provider_tokens`, not `secrets_provider_config`** — Doppler is now a first-class auth provider. Its OAuth token goes through the same path as Claude/GitHub tokens. `secrets_provider_config` only stores project/config selection.

3. **Auto-persist CLI tokens** — `resolve_snapshot` in `ProviderAuthService` detects when a broker reports Active but no stored token exists, and auto-persists it. Handles `doppler login` done outside lfd.

4. **Separate list/select API** instead of single `connect` endpoint — the UI needs to browse projects and configs before committing. Three endpoints: `GET /secrets/projects`, `GET /secrets/configs?project=X`, `POST /secrets/select`.

5. **AuthProvider expanded to all 7 providers** — Swift model now includes opencode, asana, linear, doppler (matching the Rust Provider enum). Role-based grouping is driven by `AuthProvider.role`.

## How it fits together

```
DopplerAuthBroker (Rust)
  └─ check_status / extract_token from `doppler` CLI
  └─ auto-persist into provider_tokens

secrets.rs (Rust)
  └─ get_doppler_token from provider_tokens
  └─ list_projects / list_configs / fetch_secrets via Doppler API
  └─ sync_secrets: fetch → match KEY_MAPPINGS → upsert provider_tokens
  └─ clear_secrets_credentials: remove only "via doppler" tokens

routes/secrets.rs (Rust)
  └─ GET /secrets → status
  └─ GET /secrets/projects → project list
  └─ GET /secrets/configs?project= → config list
  └─ POST /secrets/select → save config + sync
  └─ POST /secrets/sync → re-sync
  └─ DELETE /secrets → clear + disconnect

ConnectionsPanel (Swift)
  └─ ProviderGroupSection per ProviderRole
      └─ ProviderRow per provider (icon, name, status dot, action)
  └─ SecretsConfigView footer under Secrets group
      └─ project/config pickers → selectConfig → sync
      └─ key status dots → Refresh / Disconnect
```

## Risks and bottlenecks

- **Doppler API rate limits** — `list_projects` and `list_configs` fire per_page=100 without pagination. Fine for typical usage but could hit limits on large Doppler orgs.
- **No token refresh** — Doppler service tokens don't expire, but if they're revoked the user gets a 401 and must re-auth manually. The error path is clean (SecretsError::Unauthorized → 412 to client).
- **Single secrets provider** — hardcoded to Doppler. Adding a second provider would need the project/config discovery abstracted. Not urgent — Doppler is the right first target.

## What's not included

- Per-repo enable/disable toggles (infrastructure is wired — `enabledProviders` param on `ConnectionsPanel` and `ProviderGroupSection` — but no UI to persist the toggle yet)
- Portfolio-level secrets configuration (the sheet creates a standalone `SecretsProviderStore` that doesn't persist)
- Typed auth steps (design doc at `wave/concerto/05-typed-auth-methods.md` — separate work)
