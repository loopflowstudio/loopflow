# Review: DB-backed token store + credential injection hardening

## What was implemented

Replaced the Unix socket credential bridge (Swift CredentialSocketServer ↔ Rust CredentialSocketClient) with a DB-backed token store. Provider tokens (GitHub, Claude, Codex) are now extracted after CLI auth flows and stored in lfd's database. Executors inject tokens as env vars at agent launch, scoped per-agent.

Additionally: harness-specific credential filtering ensures agents only receive credentials they're authorized to use, and OpenCode model policy validation prevents routing through Claude/Codex providers via the OpenCode harness.

## Key changes

### Token store (DB layer)
- Migration `016_provider_tokens.sql`: new `provider_tokens` table with `BIGINT` for timestamp columns (Postgres-portable)
- `ProviderToken` struct + `TokenStore` trait with `get`, `upsert`, `delete`, `list` — implemented in both SQLite and Postgres backends
- Store dispatch follows the existing session/wave pattern (explicit match arms)

### Auth broker integration
- `ProviderAuthService` gains `SharedStore`: `resolve_status()` checks DB first, falls back to broker filesystem probes
- `start_auth` extracts tokens after successful auth and persists via `upsert_provider_token`
- `disconnect` deletes the token row
- Errors on DB operations are logged (not silently swallowed) via `tracing::warn`

### Token extraction per provider
- **GitHub**: parses `oauth_token` from `~/.config/gh/hosts.yml`
- **Claude**: parses `accessToken` from `~/.claude/.credentials.json`; `ClaudeAuthBroker` now takes `home_dir` for testability
- **Codex**: reads only `access_token` from `~/.codex/auth.json` — manual `api_key` entries are deliberately ignored

### Credential injection
- `provider_env_vars()` maps DB tokens to env vars: `github` → `GH_TOKEN`, `claude` → `CLAUDE_CODE_OAUTH_TOKEN`
- Codex OAuth injection intentionally deferred (token stored in DB but not injected as env)
- **DockerExecutor**: `collect_env()` now takes `program: Option<&str>`, filters both DB tokens and config-specified API keys through harness-specific allow lists
- **LocalProcessExecutor**: removes disallowed API key env vars before spawn, injects allowed DB tokens

### Harness-specific credential filtering
- `provider_env_allowed_for_program()`: GH_TOKEN allowed for all agents; CLAUDE_CODE_OAUTH_TOKEN only for `claude`
- `api_key_env_allowed_for_program()`: API keys (ANTHROPIC, OPENAI, CODEX, GEMINI, OPENCODE, MOONSHOT) are blocked for all agents except `opencode`, which only gets OPENCODE_API_KEY and MOONSHOT_API_KEY
- Normalization: program names resolved to basename+lowercase, env names uppercased

### OpenCode model policy (launch.rs + opencode.rs)
- `validate_model_policy()` rejects bare `opencode` and Claude/Codex family models routed through OpenCode
- `opencode_model()` injects `providerID`/`modelID` into turn payloads for supported variants
- Supported: `moonshotai/kimi*` and `opencode/*` (excluding claude/codex)

### Credential socket removal
- Deleted: `credential_socket.rs`, `SocketAuthBroker`, `CredentialSocketServer.swift`
- Removed socket mount from BundledDaemonManager Docker args
- Removed `LFD_CREDENTIAL_SOCKET` env var and config field
- Removed `AGENT_API_KEYS` constant from docker/mod.rs (superseded by centralized `API_KEY_ENV_NAMES`)

### Swift / Concerto
- `BundledDaemonManager`: socket lifecycle removed; container mode config simplified
- `ConcertoConfig`: enhanced parsing with YAML support and test coverage
- `ConnectionSettingsView`: UI additions for provider connections

## How it fits together

```
Auth flow:  CLI auth → broker detects success → extract_token() → DB upsert
Agent launch: executor → provider_env_vars(store) → filter by program → env inject
```

The DB is the single source of truth for tokens. Filesystem probes (gh hosts.yml, claude credentials) serve as fallback for status checks when no DB row exists. The filtering layer (provider_env_allowed + api_key_env_allowed) acts as a policy firewall between the token store and agent processes.

## Risks and considerations

- **Token encryption**: Tokens are stored as plaintext in SQLite/Postgres. Mitigated by file permissions; encryption noted as future work in migration SQL comment.
- **Codex OAuth injection deferred**: Codex tokens are extracted and stored but not injected as env vars. This means Codex agent auth still relies on mounted config dirs or manual API keys passed through credential_env config.
- **Helper containers get no API keys**: When `program` is `None` (helper containers), API key env vars from credential_env are skipped. This is intentional — helpers (git clone, worktree prep) don't need agent API keys.

## What's not included

- Proactive token refresh (wave/auth/02-proactive-refresh.md)
- `lfd install` interactive onboarding (wave/auth/04-install-onboarding.md)
- Token encryption at rest
- Codex OAuth env var injection

## Test coverage

- Token store CRUD: SQLite + Postgres backend tests
- Auth broker integration: status resolution with DB, token extraction per provider
- Credential filtering: unit tests for `api_key_env_allowed_for_program`, `provider_env_allowed_for_program`
- Codex extraction: verifies manual API keys are ignored, OAuth tokens are read
- OpenCode model policy: validates rejection of unsupported variants, acceptance of supported ones
- All existing tests pass (Rust, Python, Swift)
