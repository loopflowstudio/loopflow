---
asana_id: '1213718558926550'
linear_id: dff685b9-5630-4908-9359-de190ac02a1a
notion_id: 32af8f99-3d81-8194-b75c-c5c50bea610c
---
# Provider Auth

**Finish line:** All providers use typed, provider-specific auth flows. PM providers are OAuth-only (no API key fallbacks). Concerto renders the right UI per provider — device code, terminal-assisted, browser OAuth, or API key — not one generic "Connect" button.

## Context

Notion already shipped OAuth-only (no `api_key_env_name`, Basic-auth token exchange on port 19223). Asana still has `ASANA_ACCESS_TOKEN` and Linear still has `LINEAR_API_KEY` as API-key fallback paths. The auth story is inconsistent across providers.

On the UI side, `AuthProviderCard` was replaced by `ProviderRow` — simpler, role-grouped, with connect/disconnect and status dot. `AuthProvider` enum includes all 7 providers with a `role: ProviderRole` property. Credential detection and eager daemon startup shipped. What remains: terminal-assisted flows, typed `AuthStep` model, provider provenance badges, device-code stepper.

## What to build

### Backend: OAuth-only PM auth

1. Remove PM-provider API-key setup flows from `lfq` / `lf op`.
2. Make PM sync load stored OAuth credentials rather than PM-specific env var API keys.
3. Use the existing Asana/Linear broker path as the baseline and add any missing CLI / route cleanup so the PM experience is consistently browser-connect first.
4. Leave model-provider API-key behavior alone; this is about PM auth only.

### Frontend: Typed auth step model

Replace `AuthFlow` (which only models browser/device-code fields) with a discriminated union:

- `DeviceCodeStep { url, code, expires_at, prerequisite_message? }` — GitHub, Codex
- `BrowserStep { url, callback_kind, local_callback_port? }` — where we control the callback
- `TerminalStep { command, human_instructions, detection_targets }` — Claude especially
- `ApiKeyStep { placeholder, help_url, validate_on_submit }` — all providers
- `PrerequisiteStep { title, body, action_url?, continue_action }` — gates

Update both Swift and Rust sides. `AuthProviderStore` manages `PendingAuthStep` instead of browser-only pending flows. `ProviderRow` renders the appropriate UI per step type.

### Provider capability declarations

Each provider declares supported methods:

- **GitHub**: detect `gh auth`, device code, terminal login
- **Claude**: detect local login, terminal-assisted `claude auth login`, API key
- **Codex**: detect local login, device code, API key, terminal login
- **OpenCode Zen**: API key only

### Provider provenance display

Connected providers show where auth came from: "GitHub CLI", "Claude Code login", "ChatGPT device auth", "API key". Status endpoint returns `provenance` and `available_methods`.

## Constraints

- PM commands should not silently fall back to `ASANA_ACCESS_TOKEN`, `LINEAR_API_KEY`, or future `NOTION_API_KEY` paths.
- If a provider flow is not machine-verifiable and machine-completable, do not present it as an in-app connect ceremony. Offer "Open in Terminal" and monitor auth files.
- All status reads come from the live bundled daemon `/v0/auth` endpoint — no separate cached state.
- Keep `lf op auth` / `lfq auth` as the only local auth surface.

## Done when

- Asana and Linear PM flows connect via OAuth, API-key setup flows are gone
- `AuthFlow` replaced with tagged step model in Swift and Rust
- Provider cards render method-appropriate UI (not generic Connect)
- Terminal-assisted flow works for Claude
- Device-code stepper works for GitHub/Codex
- Provider provenance shown on connected cards
