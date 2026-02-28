# 02: API Key Fallback

**Finish line:** Users choose one auth method per provider — OAuth or API key, never both. Loopflow only forwards the credential explicitly configured in the connections panel. Onboarding drives OAuth by default; API keys are an explicit opt-in with clear billing warnings.

## What to build

### Mutual exclusivity per provider

Each provider has exactly one active credential type: `oauth` or `apikey`. Stored in the DB alongside the token. Loopflow only forwards the configured credential — never both, never guesses.

- `engine/agent.rs` currently strips `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY` from agent subprocesses. Replace blanket stripping with selective forwarding: only pass through the credential type the user explicitly chose.
- `provider_auth.rs` credential gating (`api_key_env_allowed_for_program`) updated to check the per-provider credential type setting, not just a hardcoded allowlist.
- When credential type is `oauth`: forward OAuth token, strip API key env vars.
- When credential type is `apikey`: forward API key, don't inject OAuth token.
- When neither configured: strip everything (safe default).

### Onboarding changes

OAuth is the default path. API keys are acknowledged but require explicit opt-in:

1. Detect API keys in environment
2. If found: "Found ANTHROPIC_API_KEY. API key auth bills per token — OAuth uses your subscription. We recommend OAuth."
3. Run OAuth flow as normal
4. If OAuth fails/skipped and API key exists: "Use API key for Claude? [y/N]" — default No
5. If user confirms: store credential type as `apikey`, count as connected
6. If user declines: provider stays disconnected

### Credential mapping

| Env var | Agent it enables | Billing model |
|---------|-----------------|---------------|
| `ANTHROPIC_API_KEY` | Claude Code | Pay-per-token (API) |
| `OPENAI_API_KEY` | Codex CLI | Pay-per-token (API) |
| `OPENCODE_API_KEY` | OpenCode | Pay-per-token (API) |

### `lfq auth status` changes

Show API key status alongside OAuth status:

```
┌──────────────┬──────────┬──────────────────────────────────────┐
│ provider     │ status   │ details                              │
├──────────────┼──────────┼──────────────────────────────────────┤
│ Claude       │ ✓ active │ jack@anthropic.com · expires 4h      │
│ GitHub       │ ✓ active │ @jackdoe                             │
│ Codex        │ ~ apikey │ OPENAI_API_KEY · oauth recommended   │
│ OpenCode Zen │ ✗ none   │ not connected                        │
└──────────────┴──────────┴──────────────────────────────────────┘
```

### Concerto changes

Add an "API Keys" section to auth settings, separate from "Connected Accounts" (OAuth):

- Show detected API keys (masked) and their billing model
- Nudge toward OAuth as the recommended path
- Allow entering API keys directly (for users without shell env access)

## Guardrails

API keys can silently run up large bills. `ANTHROPIC_API_KEY` in the environment caused Claude Code to bill per-token instead of using the subscription — easy to spend $1k+ before noticing. Mutual exclusivity is the primary guardrail; the rest reinforces it.

### Only forward what's configured

The core invariant: loopflow never forwards both OAuth token and API key for the same provider. The credential type in the DB is the single source of truth. `engine/agent.rs` enforces this at spawn time.

### Explicit opt-in for API keys

Default is **No** during onboarding. `lfq auth configure <provider> --credential apikey|oauth` to switch later. The DB stores the choice; re-running onboarding or connecting via OAuth flips it back.

### Persistent billing indicator

When running on API key auth, make it visible:

- `lfq auth status`: show `⚠ apikey (pay-per-token)` not just `~ apikey`
- `lfq` default output: one-line warning when any provider is on API billing
- Concerto: badge or color change on the provider status indicator
- Agent logs: log line at session start: `"using ANTHROPIC_API_KEY (pay-per-token billing)"`

### Switch to OAuth on connect

When a user connects via OAuth, flip credential type to `oauth` automatically:

1. OAuth flow completes → store token, set credential type to `oauth`
2. API key stops being forwarded for that provider
3. Print: "Switched Claude from API key to OAuth (subscription billing)"

### Spend visibility

Surface per-session cost estimates for API-key providers. See cost wave `02-api-key-fallback` integration — the cost wave needs auth-type-aware billing split.

## Open questions

- Should Concerto store user-entered API keys in the DB alongside OAuth tokens, or write them to a config file? DB is simpler but mixes two auth models.
- When both OAuth and API key exist for the same provider, should `lfq auth status` show both or just the active one (OAuth)?
- Should there be a `lfq auth apikey <provider>` command for setting keys, or is env-var-only sufficient?

## Constraints

- OAuth always takes priority over API keys when both are present.
- Never print or log API key values. Mask them ("ANTH...K3Y2") in all UI surfaces.
- The warning about billing differences must be visible during onboarding. Users should understand the cost model before proceeding.
