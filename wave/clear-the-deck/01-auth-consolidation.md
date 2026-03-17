# 01: Auth Consolidation

**Finish line:** Studio auth service deleted. lfd handles all authentication natively. Three modes: solo (local token), team (WorkOS OAuth + PKCE), CI (static LFD_AUTH_TOKEN).

## Context

lfd already has local token auth (auto-generated) and a WorkOS OAuth integration framework. The studio auth service is a separate deployment that adds complexity without capability. Provider auth (GitHub, Claude, Codex, Zen) routes through lfd's `/v0/auth/` endpoints and stores credentials in platform keychain.

The studio service handles: WorkOS OAuth dance, JWT issuance, user identity lookup, tier gating. Of these, tier gating is being deleted entirely. The rest moves into lfd.

## What to build

1. **Move OAuth into lfd.** The WorkOS PKCE flow runs inside lfd's HTTP server. JWT issuance uses lfd's existing token infrastructure. No separate service to deploy.

2. **Three auth modes.** Configuration-driven, not code-branched:
   - `solo`: local token, auto-generated on first run, current behavior unchanged
   - `team`: WorkOS OAuth, JWT, user identity — requires WorkOS client ID in config
   - `ci`: static token via `LFD_AUTH_TOKEN` env var, no browser flow

3. **Delete studio auth service.** Remove the codebase, deployment config, and any references. Update docs.

4. **Delete tier gating.** Remove tier checks, feature flags tied to tiers, and the tier model itself. Everything is available to everyone.

## Done when

- `cargo test --all` and `uv run pytest python/tests/` pass
- lfd starts in each of the three auth modes
- Studio auth service codebase and deployment config deleted
- No remaining references to tier gating
