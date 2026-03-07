# 05: Typed Auth Methods

**Finish line:** `AuthFlow` replaced with a tagged step model. Provider cards render device-code, terminal-assisted, API key, and browser flows using provider-declared capabilities — not one generic "Connect" button.

## Carried context

Phase 1 shipped credential detection and eager daemon startup:

- `FileCredentialReader` detects Claude/Codex credentials from disk files. Keychain fallback for GitHub.
- `CredentialSocketServer` serves detected credentials to the containerized daemon.
- Bundled daemon starts eagerly at app launch; `RepoState` joins the in-flight start task.
- Connection handshake skips TLS and repo discovery for bundled mode.
- Orphan worktree sidebar section removed.

What Phase 1 did NOT ship:

- Terminal-assisted auth flows (open terminal running `claude auth login`, monitor for completion)
- Typed `AuthStep` model replacing `AuthFlow`
- Provider provenance badges in the UI
- Device-code stepper UI component

## What to build

### Replace `AuthFlow` with typed step model

Replace the current `AuthFlow` (which only models browser/device-code fields) with a discriminated union:

- `DeviceCodeStep { url, code, expires_at, prerequisite_message? }` — GitHub, Codex
- `BrowserStep { url, callback_kind, local_callback_port? }` — where we control the callback
- `TerminalStep { command, human_instructions, detection_targets }` — Claude especially
- `ApiKeyStep { placeholder, help_url, validate_on_submit }` — all providers
- `PrerequisiteStep { title, body, action_url?, continue_action }` — gates

Update both Swift and Rust sides. `AuthProviderStore` manages `PendingAuthStep` instead of browser-only pending flows.

### Provider capability declarations

Each provider declares supported methods:

- **GitHub**: detect `gh auth`, device code, terminal login
- **Claude**: detect local login, terminal-assisted `claude auth login`, API key
- **Codex**: detect local login, device code, API key, terminal login
- **OpenCode Zen**: API key only

### Reusable UI components

- Device-code stepper: open browser button, large copyable code, progress/timeout, prerequisite message, "Continue in Terminal" fallback
- Terminal-assisted flow: "Sign in with Claude Code" opens terminal, card shows "Waiting for login...", daemon watches auth files, card flips on detection
- API key entry: SecureField, masked display, billing warning, "Switch to OAuth" (overlaps with 02-api-key-entry — coordinate)

### Provider provenance display

Connected providers show where auth came from: "GitHub CLI", "Claude Code login", "ChatGPT device auth", "API key". Status endpoint returns `provenance` and `available_methods`.

### Status endpoint enhancement

```json
{
  "provider": "claude",
  "status": "active",
  "credential_type": "oauth",
  "provenance": "claude_code_login",
  "last_verified_at": "2026-03-06T22:10:00Z",
  "available_methods": ["detect_local", "terminal_login_monitor", "api_key_entry"]
}
```

## Constraints

- If a provider flow is not machine-verifiable and machine-completable, do not present it as an in-app connect ceremony. Offer "Open in Terminal" and monitor auth files.
- All status reads come from the live bundled daemon `/v0/auth` endpoint — no separate cached state.
- File credential parsing is best-effort; silent failure is acceptable since terminal-assisted flows are the fallback.

## Done when

- `AuthFlow` replaced with tagged step model in Swift and Rust
- Provider cards render method-appropriate UI (not generic Connect)
- Terminal-assisted flow works for Claude
- Device-code stepper works for GitHub/Codex
- Provider provenance shown on connected cards
