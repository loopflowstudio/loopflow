# API Key Fallback + Wave Mode Refactoring

## What was implemented

Two related changes shipped together:

**1. API key auth as an alternative to OAuth.** Users can now connect providers (Claude, Codex, OpenCode Zen) using API keys stored in the DB alongside the existing OAuth flow. A new `credential_type` column on `provider_tokens` drives env var selection: Claude with OAuth gets `CLAUDE_CODE_OAUTH_TOKEN`, Claude with API key gets `ANTHROPIC_API_KEY`. One decision point in `provider_auth.rs` (`env_var_for_token`) replaces the previous scattered mapping.

New surfaces:
- `lfq auth configure <provider> --credential apikey|oauth` CLI command
- `PUT /v0/auth/{provider}/credential` HTTP endpoint
- Onboarding detects API keys in env, warns about billing, offers fallback after OAuth fails
- `lfq auth status` shows credential type (oauth vs apikey) per provider
- Swift `AuthProviderCard` shows credential type badge

**2. Wave execution mode as a first-class concept.** `Wave.flow` replaced by `Wave.mode` (loop/cron/manual), `Wave.primary_flow`, and `Wave.cron`. The `Signal::Loop`, `Signal::Once`, and `Signal::Cron` variants are removed from stimuli — scheduling lives on the wave, reactive triggers (Watch, Listen, CiFailure) remain as stimuli. `stimulus_id` is now `Option<LfdId>` throughout, since manual and loop activations have no stimulus.

## Key choices

| Decision | Rationale |
|----------|-----------|
| API keys in `access_token` column, differentiated by `credential_type` | One column, one table, no join needed. Design doc evaluated a separate `api_keys` table and rejected it. |
| `env_var_for_token` as single mapping point | Replaces three overlapping layers (agent.rs strip, provider_auth allowlist, executor filtering). Blanket strip in sync path kept for safety. |
| Mode on wave, not on stimulus | Loop/cron are execution strategies, not reactive triggers. Stimulus is now purely reactive (watch, listen, ci_failure). |
| Default flow changed from `build` to `ship-roadmap` | Aligns with the full lifecycle default for new waves. |
| `stimulus_id` nullable | Manual and loop activations have no stimulus. Avoids synthetic "once" stimuli that existed only to satisfy a NOT NULL constraint. |
| Removed `Signal::Once`, `Signal::Loop`, `Signal::Cron` | Old discriminant values (1, 2, 4) map to `Unspecified` for DB compatibility. Tested explicitly. |

## How it fits together

```
Wave.mode (loop/cron/manual)
  └─ drives scheduling: loop_ticker, cron_poller

Wave.primary_flow
  └─ default flow for activations

Stimulus (watch/listen/ci_failure)
  └─ reactive triggers only, each with optional flow override

ProviderToken.credential_type (oauth/apikey)
  └─ env_var_for_token() → correct env var per provider
  └─ Docker executor + sandbox executor use this mapping
```

## Risks and bottlenecks

- **Docker executor makes per-provider DB calls** in the credential injection loop (3 calls per container launch). Could batch with `list_provider_tokens()` and use `env_var_for_token` directly. Not a correctness issue, but adds latency to every container launch.
- **Cron last_triggered tracking** was broken (hardcoded `None`), fixed during gate by querying the most recent activation log. Less precise than the old per-stimulus tracking — if the activation log is pruned, cron waves could re-fire prematurely.
- **Migration 024 recreates `pending_activations` and `activation_log` tables** to make `stimulus_id` nullable. This drops and recreates tables — existing pending activations and activation log entries are preserved via INSERT...SELECT, but indexes are rebuilt.

## What's not included

- **Concerto ConnectionSettingsView** for API key entry (SecureField, manual key input). The design doc specifies this but it's deferred — only the badge display on AuthProviderCard shipped.
- **Encryption at rest** for API keys stored in the DB. Design doc notes this as a future concern. API keys are stored unencrypted, same as OAuth tokens.
- **Per-wave cron tracking** field. The fix uses activation log as a proxy. A dedicated `last_cron_triggered_at` column on waves would be more accurate.
