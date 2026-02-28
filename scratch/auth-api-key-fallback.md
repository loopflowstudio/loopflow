# API Key Fallback

## Problem

Users with API keys in their shell environment (ANTHROPIC_API_KEY, OPENAI_API_KEY) get silently billed per-token when loopflow launches agents. The blanket stripping in `engine/agent.rs` prevents this for the sync path, but the executor path (sandbox, local, docker) uses a hardcoded allowlist that only permits opencode keys. There's no user-facing control, no awareness of which credential type is active, and no way to deliberately use API keys when that's what you want.

Who benefits: every loopflow user with API keys in their environment. Power users who want API key auth get a first-class path. Everyone else gets protection from accidental billing.

Why now: the auth wave established DB-backed token management and background refresh. The credential type decision is the missing piece — without it, loopflow has to guess, and guessing wrong costs money.

## Approach

Add a `credential_type` column to `provider_tokens`. Every provider has exactly one active credential type: `oauth` or `apikey`. All credential forwarding reads this column. No guessing, no dual-forwarding, no hardcoded allowlists.

### Data model

One migration, one new enum, one new column.

```sql
-- migrations/017_credential_type.sql
ALTER TABLE provider_tokens ADD COLUMN credential_type TEXT NOT NULL DEFAULT 'oauth';
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth,
    ApiKey,
}
```

`ProviderToken` gains `credential_type: CredentialType`. Existing rows default to `oauth`. API keys stored via `access_token` field — same column, different meaning based on type.

### Credential forwarding

Replace the three-layer filtering (agent.rs blanket strip + provider_auth hardcoded allowlist + executor filtering) with one decision point in provider_auth.rs:

```
provider_env_for_program(program, provider, store) -> Vec<(String, String)>
```

This function reads `credential_type` from the DB and returns the correct env vars:

| credential_type | What's forwarded | What's stripped |
|-----------------|-----------------|-----------------|
| `oauth` | OAuth token (CLAUDE_CODE_OAUTH_TOKEN, etc.) | API key env vars |
| `apikey` | API key env var (ANTHROPIC_API_KEY, etc.) | OAuth token |
| none in DB | nothing | everything |

The mapping from provider to env vars:

| Provider | OAuth env var | API key env var |
|----------|--------------|-----------------|
| Claude | CLAUDE_CODE_OAUTH_TOKEN | ANTHROPIC_API_KEY |
| Codex | (not injected today) | OPENAI_API_KEY |
| OpenCode Zen | OPENCODE_API_KEY | OPENCODE_API_KEY |
| GitHub | GH_TOKEN | (no API key path) |

**engine/agent.rs**: Keep the blanket `env_remove` calls. The sync path (`lf design`, `lf agent`, etc.) has no DB access — `launch_agent` is synchronous, takes `&AgentConfig` / `&ProcessConfig` / `&AgentCapabilities`, none of which carry a store handle. Adding DB access here would require pulling `lfd::store` into the engine crate, creating a tokio runtime for a sync function, and knowing the DB path from lfd's config. The blanket strip is the right semantic for this path: prevent accidental API key forwarding to interactive agent sessions. Users running `lf` directly manage their own env.

**Executors (sandbox, local, docker)**: Replace `api_key_env_allowed_for_program` + `provider_env_allowed_for_program` with the unified `provider_env_for_program`. The executor doesn't decide — it asks provider_auth, which reads the DB.

**Docker cached credentials**: The Docker executor's third credential source (`cached_credentials` from `LFD_CREDENTIAL_SOCKET`) currently maps `("claude", "ANTHROPIC_API_KEY")` — injecting an OAuth-origin token as an API key env var. This path must also honor `credential_type`. The cached credentials tier should use `provider_env_for_program` like everything else, mapping to the correct env var based on credential type rather than hardcoding the provider→env_var mapping.

### Onboarding

Modify `run_install_onboarding` in `onboarding.rs`:

For each agent provider (Claude, Codex, OpenCode):
1. Check env for the provider's API key
2. If found: print warning — `"Found ANTHROPIC_API_KEY. API key auth bills per token. OAuth uses your existing subscription. We recommend OAuth."`
3. Run OAuth flow as normal
4. If OAuth succeeds: store with `credential_type: oauth`, done
5. If OAuth fails/skipped AND API key exists: prompt `"Use API key for Claude? [y/N]"` (default No)
6. If user confirms: upsert token with `credential_type: apikey`, `access_token` = env var value
7. If user declines: provider stays disconnected

Non-interactive mode (`--no-interactive`): skip API key prompt, default to OAuth only. API keys require explicit opt-in.

### CLI changes

**`lfq auth status`**: Show credential type in the details column.

```
┌──────────────┬──────────┬──────────────────────────────────────┐
│ provider     │ status   │ details                              │
├──────────────┼──────────┼──────────────────────────────────────┤
│ Claude       │ ✓ oauth  │ jack@anthropic.com · expires 4h      │
│ GitHub       │ ✓ oauth  │ @jackdoe                             │
│ Codex        │ ⚠ apikey │ OPEN...KEY · pay-per-token            │
│ OpenCode Zen │ ✗ none   │ not connected                        │
└──────────────┴──────────┴──────────────────────────────────────┘
```

Status column shows the credential type, not just active/none. The `⚠` for apikey is the persistent billing indicator.

**`lfq auth configure <provider> --credential oauth|apikey`**: New subcommand. Switches credential type for a connected provider. When switching to apikey: reads from env, stores in DB, warns about billing. When switching to oauth: requires existing OAuth token in DB (run `lfq auth <provider>` first if not).

**HTTP API**: Add `credential_type` field to `AuthProviderStatusDto`. The Python models and table rendering follow.

### Concerto changes

**AuthProviderCard**: Show credential type badge next to status indicator. When apikey: use `statusWarning` color and show "API Key" label. The existing "Connected" label becomes "OAuth" when credential_type is oauth.

**ConnectionSettingsView**: Add "API Keys" section below the provider cards. For each provider that supports API keys (Claude, Codex, OpenCode):
- If env key detected: show masked value, "Use this key" button
- SecureField for manual entry
- Billing model warning inline
- "Switch to OAuth" nudge

**AuthProvider model**: Add `credentialType: CredentialType?` to `AuthProviderStatus`. New `CredentialType` enum (`.oauth`, `.apikey`).

### Auto-switch on OAuth connect

When `upsert_provider_token` is called with a token from an OAuth flow, set `credential_type = 'oauth'` regardless of previous value. If previous was `apikey`, log: `"Switched {provider} from API key to OAuth (subscription billing)"`.

This happens in the existing auth flow — no new code path needed, just a credential_type assignment in the upsert.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate `api_keys` table | Clean separation of OAuth and API key storage | Two tables to coordinate, need a "which is active" flag elsewhere, more complex queries. One column on the existing table is simpler. |
| Env-only API keys (no DB storage) | API keys never touch the DB, smallest attack surface | Concerto users without shell access can't enter keys. The DB already stores OAuth tokens unencrypted — API keys don't change the security posture. |
| Keep hardcoded allowlist, add a config file | No schema change | Config file is another source of truth to sync. The DB is already the authority for provider state. |

## Key decisions

**API keys stored in DB via `access_token` column.** Same field as OAuth tokens, differentiated by `credential_type`. The DB already stores OAuth tokens unencrypted; API keys don't change the security posture. Encryption is a separate concern tracked in the auth wave.

**One decision point for executors, blanket strip for sync path.** Today's filtering is split across agent.rs (blanket strip), provider_auth.rs (hardcoded allowlist), and executors (double-check). The new design keeps the blanket strip in agent.rs (sync path has no DB access) and puts the credential-type-aware decision in `provider_auth` for the executor path. Two layers, each with clear ownership — not three layers doing overlapping work.

**Auto-switch to OAuth is implicit.** Connecting via OAuth automatically sets credential_type to oauth. No confirmation prompt — if you ran the OAuth flow, you want OAuth. Switching back to apikey requires explicit `lfq auth configure`.

**Only show the active credential in status.** If a user has both an OAuth token and an API key available, `lfq auth status` shows whichever is active (determined by credential_type). Showing both invites confusion about which one loopflow is using.

**Concerto stores API keys in DB, not keychain.** The keychain path exists for OAuth tokens read from vendor CLI auth files. User-entered API keys go to the DB via the HTTP API, consistent with how all other provider state is managed.

## Scope

In scope:
- `credential_type` column, migration, enum
- Unified credential forwarding in provider_auth.rs
- Keep blanket stripping in engine/agent.rs (sync path, no DB access)
- Collapse Docker executor's three-tier credential injection to use `provider_env_for_program`
- Onboarding API key detection and opt-in prompt
- `lfq auth status` credential type display
- `lfq auth configure` subcommand
- HTTP API credential_type field
- AuthProviderCard credential type badge in Concerto
- API key entry in ConnectionSettingsView
- Auto-switch on OAuth connect
- Agent log line at session start for apikey providers

Out of scope:
- Token encryption at rest (separate auth wave item)
- Per-session cost estimates (cost wave integration — needs auth-type-aware billing split)
- Multi-user token isolation (wave vision: "not here")
- API key rotation/refresh (API keys don't expire in the same way)
- `lfq` default output billing warning (polish — add after core works)

## Done when

1. `cargo test --all` passes with new credential_type tests
2. `lfq auth status` shows credential type for each provider
3. `lfq auth configure claude --credential apikey` switches to API key, `lfq auth status` reflects it
4. Onboarding with ANTHROPIC_API_KEY in env shows billing warning and offers opt-in
5. Executor-spawned agent with `credential_type=oauth` receives OAuth token, not API key
6. Executor-spawned agent with `credential_type=apikey` receives API key, not OAuth token
7. Executor-spawned agent with no credential receives neither
8. Sync path (`lf agent`) still strips API keys unconditionally (no regression)
9. Docker executor's cached credential path honors `credential_type`
10. Connecting via OAuth after apikey auto-switches to oauth with a logged message
11. All existing tests pass — no regression in providers that don't use API keys
