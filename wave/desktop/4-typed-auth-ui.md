# Typed auth UI per provider

**Finish line:** Concerto renders the right auth UI per provider — device code, terminal-assisted, browser OAuth, or API key — driven by a typed `AuthStep` model, not a single generic Connect button. Connected cards show provenance ("GitHub CLI", "Claude Code login", "ChatGPT device auth", "API key").

## Context

Backend auth is now consistent: PM providers (Asana, Linear, Notion) are OAuth-only, model providers (Claude, Codex, OpenCodeZen) keep API-key fallback, and `lf op auth configure` rejects PM providers with an actionable error pointing at OAuth. `AuthProviderCard` was already replaced by `ProviderRow` — simpler, role-grouped, with connect/disconnect and status dot. `AuthProvider` enum includes all 7 providers with a `role: ProviderRole` property. Credential detection and eager daemon startup shipped.

What remains is the UI side: `AuthFlow` only models browser/device-code fields, so every provider gets the same generic "Connect" button regardless of how it actually authenticates. The result feels broken for providers whose real flow is terminal-assisted (Claude) or device-code (GitHub/Codex).

## What to build

### Typed auth step model

Replace `AuthFlow` with a discriminated union:

- `DeviceCodeStep { url, code, expires_at, prerequisite_message? }` — GitHub, Codex
- `BrowserStep { url, callback_kind, local_callback_port? }` — where we control the callback (Asana, Linear, Notion)
- `TerminalStep { command, human_instructions, detection_targets }` — Claude especially
- `ApiKeyStep { placeholder, help_url, validate_on_submit }` — model providers
- `PrerequisiteStep { title, body, action_url?, continue_action }` — gates

Update both Swift and Rust sides. `AuthProviderStore` manages `PendingAuthStep` instead of browser-only pending flows. `ProviderRow` renders the appropriate UI per step type.

### Provider capability declarations

Each provider declares supported methods:

- **GitHub**: detect `gh auth`, device code, terminal login
- **Claude**: detect local login, terminal-assisted `claude auth login`, API key
- **Codex**: detect local login, device code, API key, terminal login
- **OpenCode Zen**: API key only
- **Asana / Linear / Notion**: browser OAuth only (backend already enforces this)

### Provider provenance display

Connected providers show where auth came from: "GitHub CLI", "Claude Code login", "ChatGPT device auth", "API key". Status endpoint returns `provenance` and `available_methods`.

## Constraints

- If a provider flow is not machine-verifiable and machine-completable, do not present it as an in-app connect ceremony. Offer "Open in Terminal" and monitor auth files.
- All status reads come from the live bundled daemon `/v0/auth` endpoint — no separate cached state.
- Keep `lf op auth` / `lfq auth` as the only local auth surface; Concerto is a UI over the same daemon endpoints.
- Do not regress the OAuth-only PM backend — `lf op auth configure asana|linear|notion` already errors with an actionable OAuth message; the UI should not offer API-key entry for those providers.

## Done when

- `AuthFlow` replaced with tagged step model in Swift and Rust
- Provider cards render method-appropriate UI (not generic Connect)
- Terminal-assisted flow works for Claude
- Device-code stepper works for GitHub/Codex
- Provider provenance shown on connected cards
- PM provider rows offer browser OAuth only (matches backend enforcement)
