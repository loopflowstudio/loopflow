# 03: Credential Injection

Status: **shipped**

Executors pull tokens from the DB and inject them into agent processes at launch time.

## What to build

### TokenProvider helper

A standalone function (not a trait) shared by both executors:

```rust
pub async fn provider_env_vars(store: &Store) -> Vec<(String, String)>
```

Reads all provider tokens from the DB, maps each to the agent's expected env var:
- Claude → `CLAUDE_CODE_OAUTH_TOKEN` (preferred over writing credential files)
- GitHub → `GH_TOKEN`
- Codex → `OPENAI_API_KEY` or `CODEX_API_KEY`

Returns empty vec if no tokens are stored (agents fall back to their own auth).

### LocalProcessExecutor changes

Before spawning the agent command, call `provider_env_vars(store)` and inject the results via `command.env(key, value)`. This replaces the current behavior where agents inherit (or don't inherit) the host's env.

Remove the unconditional `env_remove("ANTHROPIC_API_KEY")` stripping in the sync launch path — instead, only strip if we're injecting a replacement from the DB. If no DB token exists, let the host env through.

### DockerExecutor changes

In `collect_env()`, call `provider_env_vars(store)` and merge results into the env map. DB tokens take precedence over host env vars.

## Constraints

- Injection is additive. If no DB token exists for a provider, the executor doesn't strip the host env var — existing host-auth workflows keep working.
- Don't write credential files into the executor's filesystem. Env vars are cleaner and don't persist after the process exits.
- `provider_env_vars` is a free function in `provider_auth.rs`, not a method on a service.

## Validation

```bash
cargo test -p loopflow executor
cargo test -p loopflow provider_auth
```

## Done when

- Local executor injects DB tokens into agent processes
- Docker executor injects DB tokens into container env
- Agent runs succeed in a container with no host-mounted credential files
- Host-auth fallback works when no DB tokens exist
