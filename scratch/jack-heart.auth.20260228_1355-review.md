# Review: API Key Fallback Design

## What was implemented

Design doc for credential type management. Moved `wave/auth/02-api-key-fallback.md` to `scratch/auth-api-key-fallback.md` with full elaboration — data model, credential forwarding architecture, onboarding flow, CLI/HTTP/Concerto changes, and acceptance criteria.

No code changes. This is a design-only branch.

## Key choices

**Single column on `provider_tokens` over separate table.** `credential_type TEXT NOT NULL DEFAULT 'oauth'` on the existing table. One source of truth, no coordination between tables. The DB already stores OAuth tokens unencrypted, so API keys don't change the security posture.

**Keep the blanket strip in `engine/agent.rs`.** The sync path (`lf design`, `lf agent`) has no DB access — `launch_agent` is synchronous, takes `&AgentConfig`/`&ProcessConfig`/`&AgentCapabilities`, none of which carry a store handle. The blanket strip is the right semantic: interactive agent sessions shouldn't inherit shell API keys. The design correctly avoids pulling `lfd::store` into the engine crate.

**Unified `provider_env_for_program` replaces three-layer filtering.** Today: agent.rs blanket strip + provider_auth hardcoded allowlist (`api_key_env_allowed_for_program` + `provider_env_allowed_for_program`) + executor filtering. After: agent.rs blanket strip (sync path) + `provider_env_for_program` reads DB (executor path). Two layers with clear ownership.

**OAuth auto-switch is implicit.** Connecting via OAuth flips `credential_type` to `oauth` without confirmation. If you ran the OAuth flow, you want OAuth. Switching back to apikey requires explicit `lfq auth configure`.

## How it fits together

Migration adds `credential_type` to `provider_tokens`. All executor credential injection flows through `provider_env_for_program(program, provider, store)`, which reads the DB and returns the correct env vars. Onboarding detects API keys in the environment, warns about billing, and offers opt-in after OAuth fails. CLI and Concerto surfaces show the active credential type.

## Risks and bottlenecks

**API key stored in `access_token` column.** Works, but the column now has two meanings depending on `credential_type`. If encryption-at-rest lands later, it needs to know which values are API keys vs OAuth tokens for different handling. The `credential_type` column makes this lookup trivial, so the risk is low.

**Docker cached credentials path.** The design correctly identifies that `cached_credentials` from `LFD_CREDENTIAL_SOCKET` hardcodes `("claude", "ANTHROPIC_API_KEY")`. This path needs careful refactoring — it's a third injection mechanism that the unified function must replace. Implementation should test this path explicitly.

**OpenCode uses the same env var for OAuth and API key.** `OPENCODE_API_KEY` serves both purposes. The design handles this (same env var regardless of `credential_type`), but it means the env var name doesn't signal which auth type is active. The status display compensates.

## What's not included

- Token encryption at rest (separate auth wave item)
- Per-session cost estimates (cost wave integration)
- `lfq` default output billing warning (polish pass after core)
- API key rotation/refresh

## Verdict

Design doc is accurate against the codebase and complete enough to implement from. The acceptance criteria in "Done when" are specific and testable. Ship the design, proceed to implement.
