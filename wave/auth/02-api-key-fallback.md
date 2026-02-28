# 02: API Key Fallback

**Finish line:** Users choose one auth method per provider — OAuth or API key, never both. Loopflow only forwards the credential explicitly configured in the DB. Onboarding drives OAuth by default; API keys are an explicit opt-in with clear billing warnings.

## What to build

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

Provider-to-env-var mapping:

| Provider | OAuth env var | API key env var |
|----------|--------------|-----------------|
| Claude | CLAUDE_CODE_OAUTH_TOKEN | ANTHROPIC_API_KEY |
| Codex | (not injected today) | OPENAI_API_KEY |
| OpenCode Zen | OPENCODE_API_KEY | OPENCODE_API_KEY |
| GitHub | GH_TOKEN | (no API key path) |

**engine/agent.rs**: Keep the blanket `env_remove` calls. The sync path has no DB access — `launch_agent` is synchronous, takes `&AgentConfig` / `&ProcessConfig` / `&AgentCapabilities`, none of which carry a store handle. The blanket strip is the right semantic: prevent accidental API key forwarding to interactive agent sessions.

**Executors (sandbox, local, docker)**: Replace `api_key_env_allowed_for_program` + `provider_env_allowed_for_program` with the unified `provider_env_for_program`. The executor doesn't decide — it asks provider_auth, which reads the DB.

**Docker cached credentials**: The Docker executor's `cached_credentials` from `LFD_CREDENTIAL_SOCKET` currently maps `("claude", "ANTHROPIC_API_KEY")` — injecting an OAuth-origin token as an API key env var. This path must also use `provider_env_for_program`, mapping to the correct env var based on credential type rather than hardcoding the provider→env_var mapping. This is a third injection mechanism that the unified function must replace — test it explicitly.

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

**`lfq auth status`**: Show credential type in the status column.

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

**`lfq auth configure <provider> --credential oauth|apikey`**: New subcommand. Switches credential type for a connected provider. When switching to apikey: reads from env, stores in DB, warns about billing. When switching to oauth: requires existing OAuth token in DB (run `lfq auth <provider>` first if not).

**HTTP API**: Add `credential_type` field to `AuthProviderStatusDto`. Python models and table rendering follow.

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

## Key decisions

**API keys stored in DB via `access_token` column.** Same field as OAuth tokens, differentiated by `credential_type`. The `access_token` column now has two meanings depending on `credential_type` — if encryption-at-rest lands later, it needs to check `credential_type` for different handling. The column makes this lookup trivial.

**One decision point for executors, blanket strip for sync path.** Two layers with clear ownership — not three doing overlapping work.

**Auto-switch to OAuth is implicit.** No confirmation prompt — if you ran the OAuth flow, you want OAuth. Switching back to apikey requires explicit `lfq auth configure`.

**Only show the active credential in status.** Showing both invites confusion about which one loopflow is using.

**Concerto stores API keys in DB, not keychain.** Consistent with how all other provider state is managed.

**OpenCode uses the same env var for OAuth and API key.** `OPENCODE_API_KEY` serves both purposes — the env var name doesn't signal which auth type is active. The status display compensates.

## Alternatives rejected

| Approach | Why not |
|----------|---------|
| Separate `api_keys` table | Two tables to coordinate, more complex queries. One column on the existing table is simpler. |
| Env-only API keys (no DB storage) | Concerto users without shell access can't enter keys. DB already stores OAuth tokens unencrypted. |
| Keep hardcoded allowlist, add a config file | Config file is another source of truth to sync. DB is already the authority. |

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
