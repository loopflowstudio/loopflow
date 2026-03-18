# API Key Entry via Secrets Providers

## Problem

API key auth works from CLI but there's no UI for it. Manual key paste is fragile — keys rotate, get stale, live in plaintext in random places. Users need a secrets provider, not a text field.

## Approach

Abstract secrets provider concept. Connect a secrets provider via OAuth, lfd fetches API keys, provider credentials populate automatically. Concerto shows which providers have keys and which don't.

Doppler is the first implementation. The abstraction supports adding 1Password, Vault, etc. later.

### Flow

1. User connects Doppler — OAuth device flow, same pattern as GitHub/Claude/Codex
2. lfd uses the Doppler token to fetch secrets from the user's project/config
3. Keys matching known providers (`ANTHROPIC_API_KEY` → Claude, `OPENAI_API_KEY` → Codex) are stored as API key credentials via the existing `configure_credential` path
4. Provider cards in Concerto light up — status dot, "API Key" label, credential type badge

### Model changes

**`SecretsProvider` trait (Rust) / protocol (Swift)**

```rust
// Rust
#[async_trait]
pub trait SecretsProvider: Send + Sync {
    fn id(&self) -> &str;                    // "doppler", "1password", etc.
    fn display_name(&self) -> &str;
    async fn fetch_secrets(&self, token: &str, config: &SecretsConfig) -> Result<HashMap<String, String>>;
}

pub struct SecretsConfig {
    pub project: Option<String>,   // Doppler: project name
    pub config: Option<String>,    // Doppler: config name (dev, staging, etc.)
    pub vault: Option<String>,     // 1Password: vault name (future)
}
```

```swift
// Swift — for UI display, not fetching (lfd owns sync)
public struct SecretsProviderStatus: Codable, Sendable, Identifiable {
    public let provider: String           // "doppler"
    public let displayName: String        // "Doppler"
    public let connected: Bool
    public let config: SecretsConfig?     // project/config if set
    public let suppliedKeys: [SuppliedKey]  // which provider keys were found
}

public struct SuppliedKey: Codable, Sendable {
    public let envVar: String             // "ANTHROPIC_API_KEY"
    public let targetProvider: AuthProvider  // .claude
    public let present: Bool              // found in secrets config
}
```

**Known key mappings** (hardcoded, provider-agnostic):

| Env var | Target provider |
|---------|----------------|
| `ANTHROPIC_API_KEY` | `.claude` |
| `OPENAI_API_KEY` | `.codex` |

**Doppler implementation**

Implements `SecretsProvider`. OAuth device flow for auth (same pattern as GitHub/Claude/Codex). Fetches secrets via `GET /v3/configs/config/secrets` with the Doppler token.

Doppler organizes secrets into projects → configs. After OAuth, the user sets project/config via CLI or Concerto picker.

**Secrets sync**

After any secrets provider auth completes, lfd runs sync:

1. Call `fetch_secrets` on the connected provider
2. Match returned keys against the known mapping table
3. Store matched keys via the existing `configure_credential` internal path
4. Broadcast auth events so Concerto updates live

Sync runs on:
- Initial provider connect
- Concerto foreground / reconnect (lightweight — re-fetch and diff)
- Manual "Refresh" action in UI

**Provider/config selection**

Start simple: `lf auth doppler` connects OAuth, then `lf auth doppler --project X --config Y` sets the source. Concerto shows the current project/config and a picker if multiple are available.

### View changes

**Secrets section in Connection Settings**

Below provider cards, a secrets provider section. Provider-agnostic — shows whichever `SecretsProviderStatus` is connected.

```
Secrets
┌─────────────────────────────────┐
│ Doppler — loopflow / dev        │
│                    [Disconnect] │
│                                 │
│ ✓ Claude    ANTHROPIC_API_KEY   │
│ ✗ Codex     OPENAI_API_KEY      │
│             (not in config)     │
│                       [Refresh] │
└─────────────────────────────────┘
```

When no secrets provider is connected:

```
Secrets
┌─────────────────────────────────┐
│ Connect a secrets provider to   │
│ supply API keys automatically.  │
│                                 │
│ [Connect Doppler]               │
└─────────────────────────────────┘
```

Shows which expected keys are present and which are missing. Diagnostic — "why isn't Codex connected?" is answered here.

**Provider cards for Claude/Codex**

No changes. They already show `credentialType == .apikey` with the warning dot and "API Key" label. When secrets sync pushes a key, the card reflects it automatically.

On iOS, same content in a `Section("Secrets")` in the `Form`.

### Rust changes

- `SecretsProvider` trait in `lfd::auth::secrets`
- `DopplerSecretsProvider` implementation
- Add `Doppler` variant to `Provider` enum (or keep secrets providers in a separate registry — TBD based on how auth machinery is structured)
- Doppler OAuth device flow handler
- Secrets sync module: `lfd::auth::secrets::sync` — provider-agnostic, takes any `SecretsProvider`
- New endpoints:
  - `GET /v0/secrets/status` — returns `SecretsProviderStatus`
  - `POST /v0/secrets/sync` — triggers manual re-sync
  - `PUT /v0/secrets/config` — sets project/config
- Auth endpoints reuse existing pattern for Doppler OAuth

### CLI

```bash
lf auth doppler                              # OAuth device flow
lf auth doppler --project loopflow --config dev  # set source
lf auth doppler --sync                       # manual re-sync
lf auth doppler --disconnect                 # remove Doppler + keys it supplied
```

## Alternatives considered

| Approach | Why not |
|----------|---------|
| Manual SecureField paste | Keys go stale, no rotation story, plaintext in UI. Secrets provider handles lifecycle. |
| Env var sniffing (`ProcessInfo`) | Fragile — depends on how Concerto was launched. Explicit provider connection is better. |
| Doppler CLI subprocess (`doppler secrets get`) | Requires CLI installed. API call from lfd is self-contained. |
| Doppler-specific code without abstraction | We know we'll want 1Password, Vault, etc. The trait is cheap and saves a rewrite. |

## Key decisions

**Abstract `SecretsProvider`, Doppler first.** The trait is cheap — `id`, `display_name`, `fetch_secrets`. Adding 1Password later is a new struct, not a rewrite.

**No manual fallback.** Secrets come from providers, not text fields. One path, well-supported.

**OAuth device flow, same as other providers.** Consistent auth UX. No special setup beyond connecting.

**lfd owns the sync.** Secrets flow through lfd, not Concerto. Concerto just shows status. This keeps the mobile client thin and the server authoritative.

**Project/config is explicit.** Auto-detection is nice but ambiguous with multiple projects. Start with explicit config, add auto-detect later.

**Disconnect removes supplied keys.** When a secrets provider is disconnected, the API keys it supplied are cleared. Clean break, no orphaned credentials.

## Scope

**In:**
- `SecretsProvider` trait (Rust) and `SecretsProviderStatus` model (Swift)
- `DopplerSecretsProvider` implementation
- Doppler OAuth device flow (Rust handler + Swift auth flow)
- Secrets sync module in lfd (provider-agnostic fetch, match to harness providers, store)
- `/v0/secrets/` endpoints (status, sync, config)
- `lf auth doppler` CLI command
- Secrets section in Concerto Connection Settings (both platforms)
- Key availability summary (which providers have keys, which don't)

**Out:**
- Other secrets providers (1Password, Vault — future implementations of the trait)
- Automatic rotation/re-sync on a timer (manual + reconnect sync is enough for now)
- Doppler project creation from Concerto
- Manual SecureField key entry

## Done when

- `cargo test` passes with `SecretsProvider` trait tests and Doppler sync tests
- `swift test --package-path swift` passes with `SecretsProviderStatus` model tests
- OAuth device flow connects to Doppler and stores token
- Secrets sync populates Claude/Codex credentials from Doppler config
- Concerto shows secrets provider status and key availability
- Disconnecting secrets provider clears supplied keys
- Both platforms render correctly
