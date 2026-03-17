---
layout: default
title: lfd Daemon Reference
---

# lfd Daemon Reference

`lfd` runs the loopflow daemon: HTTP API, wave scheduling, agent sessions, and CI/webhook integration.

## Choose a deployment shape

### Native (default)

Use native mode for local or single-user setups on macOS/Linux.

- Storage: sqlite
- Auth: local session token
- Executor: local process executor
- Config: none required

```bash
lfd install
lfd serve
```

`lfd` writes the local session token to `~/.lf/session-token`. `lfq` and Concerto read it automatically for loopback connections.

### Container

Use container mode for remote or shared hosts.

- Storage: postgres
- Auth: studio
- Executor: Docker
- Config: set `mode: container`, then install or run `lfd`

```yaml
# ~/.lf/lfd.yaml
mode: container
```

```bash
lfd install
```

`LFD_MODE=container` is still available as a process override, but `~/.lf/lfd.yaml` is the real mode-selection path for installed services.

The blessed container path is Docker. `executor.sandbox` remains an experimental override documented in the configuration reference below, not part of the main deployment story.

## Run the daemon

```bash
lfd serve
```

The default listen address is `127.0.0.1:2486`. Override it with `LFD_HTTP_ADDR`.

## Install

```bash
lfd install
lfd install --force
```

`--force` tears down the conflicting backend first, then reinstalls for the configured mode.

## Uninstall

```bash
lfd uninstall
```

## Migrations

```bash
lfd migrate
lfd migrate --status
```

`lfd migrate` uses the configured mode to choose the backend (`sqlite` for native, `postgres` for container). `LFD_DATABASE_URL` is required for postgres migrations.

## Authentication transport

Send credentials in the `Authorization` header:

```bash
curl -H "Authorization: Bearer <token>" "$LFD_ADDR/status"
```

`lfd` rejects malformed authorization values before provider validation. Use `Bearer <token>` with a non-empty token (max 4096 bytes) and no embedded whitespace or control characters.

`lfd` also rejects auth-like query parameters (`token`, `api_key`, `secret`, and similar) with `400 Bad Request`.

## Configuration reference

`lfd` reads `~/.lf/lfd.yaml` and then applies environment-variable overrides.

### Environment variables

Shape selection and daemon settings:

```bash
LFD_MODE          # native or container
LFD_HTTP_ADDR     # listen address (default 127.0.0.1:2486)
LFD_DB_PATH       # sqlite path override
LFD_DATABASE_URL  # postgres URL for container mode
LFD_MAX_SLOTS     # concurrent run slots
```

Auth tuning within a shape:

```bash
LFD_AUTH_MODE     # local or studio
LFD_AUTH_TOKEN    # optional session-token override
```

Executor tuning within a shape:

```bash
LFD_EXECUTOR_CREDENTIALS_ENV
LFD_EXECUTOR_CREDENTIALS_MOUNTS
LFD_EXECUTOR_IMAGE
LFD_EXECUTOR_AGENT_TIMEOUT
LFD_EXECUTOR_LIMITS_MEMORY
LFD_EXECUTOR_LIMITS_MEMORY_SWAP
LFD_EXECUTOR_LIMITS_CPU_QUOTA
LFD_EXECUTOR_LIMITS_PIDS_LIMIT
```

GitHub + HTTP security:

```bash
LFD_GITHUB_WEBHOOK_SECRET
LFD_GITHUB_TOKEN
LFD_HTTP_MAX_JSON_BODY_BYTES
LFD_HTTP_MAX_HOOK_BODY_BYTES
LFD_HTTP_MAX_WS_FRAME_BYTES
LFD_HTTP_MAX_WS_MESSAGE_BYTES
LFD_HTTP_MAX_WS_QUEUE
LFD_HTTP_MAX_WS_MALFORMED
LFD_HTTP_AUTH_FAILURES_PER_MINUTE
LFD_HTTP_TRUSTED_PROXY_CIDRS
```

### YAML

```yaml
mode: native # native (default) or container

auth:
  mode: local # tuning knob within the selected shape
  token: bundled-session-token # optional override for embedded launches
  base_url: https://auth.loopflow.studio # used by studio mode

executor:
  sandbox: false # experimental override; blessed container executor is Docker
  image: loopflow/agent:latest
  agent_timeout: 45m
  limits:
    memory: 8589934592
    memory_swap: 8589934592
    cpu_quota: 400000
    pids_limit: 1024
  credentials:
    env: ["ANTHROPIC_API_KEY", "CODEX_API_KEY"]
    mounts:
      - claude
      - codex
      - ssh

github:
  webhook_secret: your-webhook-secret
  token: ghp_xxx

http_security:
  max_json_body_bytes: 1048576
  max_hook_body_bytes: 262144
  max_ws_frame_bytes: 65536
  max_ws_message_bytes: 262144
  max_ws_queue: 256
  max_ws_malformed: 3
  auth_failures_per_minute: 12
  trusted_proxy_cidrs: []
```

`mode` selects a strict profile. `service_manager`, `runtime_backend`, `storage`, and `executor.type` are derived from the mode and rejected if set directly.

`auth.*`, `executor.*`, and `http_security.*` tune the selected shape. They are not separate deployment profiles.

In container mode, `auth.mode=local` without an explicit `auth.token` is promoted to `studio` so the blessed remote shape stays coherent.

### Credential mounts

`executor.credentials.mounts` accepts named, allowlisted mounts only:

- `claude` → `~/.claude`
- `codex` → `~/.codex`
- `gh` → `~/.config/gh`
- `gemini` → `~/.config/gemini`
- `gitconfig` → `~/.gitconfig`
- `ssh` → `~/.ssh`
- `gnupg` → `~/.gnupg`

Mounts are read-only inside the container. Raw `host:container` strings are rejected.

### Compose overrides

`lfd install` generates `~/.lf/docker-compose.yml`. Do not edit it directly.

To layer local changes on top, create `~/.lf/docker-compose.override.yml`:

```yaml
services:
  lfd:
    ports:
      - "3000:2486"
    environment:
      - EXTRA_VAR=value
  postgres:
    command: postgres -c log_statement=all
```

## Query + manage waves

Examples below use `$LFD_ADDR`:

```bash
export LFD_ADDR=http://127.0.0.1:2486
```

Use `lfq` for CLI workflows and `loopflow` for Python orchestration:

```bash
lfq list
lfq logs engbot
```

```python
import loopflow.api as loopflow

loopflow.create_wave("engbot", repo=".")
loopflow.run_wave("engbot")
```

Chord-waves are regular waves whose `area` points at other wave directories:

```bash
curl -s -X POST "$LFD_ADDR/v0/waves" \
  -H "Content-Type: application/json" \
  -d '{"repo":"'"$(pwd)"'","name":"redesign"}'

curl -s "$LFD_ADDR/v0/waves/redesign"
```

The `wave/redesign/redesign.yaml` file is the source of truth for member waves.

## Agent sessions API

Create a session:

```bash
curl -s -X POST "$LFD_ADDR/v0/sessions" \
  -H "Content-Type: application/json" \
  -d "{
    \"harness\": \"claude\",
    \"wave_run_id\": \"run_abc\",
    \"step\": \"design\",
    \"repo_root\": \"$(pwd)\",
    \"directions\": [\"clarity\"],
    \"agent\": \"claude-sonnet-4-6\",
    \"cwd\": \"$(pwd)\"
  }"
```

Send input:

```bash
curl -s -X POST "$LFD_ADDR/v0/sessions/<session_id>/input" \
  -H "Content-Type: application/json" \
  -d '{"content":"fix the failing tests"}'
```

`repo_root` must point to a local repo containing `.lf/`, and `cwd` must resolve inside that repo root when set.

Stream events:

```bash
curl -N "$LFD_ADDR/v0/sessions/<session_id>/events"
```

Session streams include metering events:

- `context_snapshot`
- `turn_usage`

Supported harnesses: `codex`, `claude`, `opencode`.

Stop the session:

```bash
curl -s -X DELETE "$LFD_ADDR/v0/sessions/<session_id>"
```

## Wave schemas

```bash
curl -s "$LFD_ADDR/v0/wave/schemas?repo=$(pwd)" | jq '.data[].name'
```

```bash
curl -s -X POST "$LFD_ADDR/v0/waves" \
  -H "Content-Type: application/json" \
  -d "{\"repo\":\"$(pwd)\",\"schema\":\"scan\"}"
```

Use explicit refs when names collide:

```json
{"schema":"builtin://scan"}
{"schema":"file:///abs/path/to/repo/wave/scan/scan.yaml"}
```

## GitHub CI auto-fix

```bash
POST /v0/hooks/github
POST /v0/waves/{wave_id}/check-ci
```

Set `github.webhook_secret` or `LFD_GITHUB_WEBHOOK_SECRET` before enabling the webhook. `lfd` verifies `X-Hub-Signature-256` and ignores unsigned requests.

CI fix agents run the built-in `ci-fix` step.

## Activation telemetry

```bash
GET /v0/waves/{wave_id}/activations?limit=50
```

Activation sources are `poll`, `push`, `listen`, or `manual`.

WebSocket streams also emit:

- `activation_queued`
- `activation_coalesced`
- `activation_dropped`

## Stacked PR queue state

```bash
curl -s "$LFD_ADDR/v0/wave_runs?wave_id=<wave_id>&order=stack" | jq '.data[] | {id, stack_position, queue_role, queue_block_reason, next_action}'
```

`/v0/wave_runs` includes:

- `queue_role`: `ready | draft | blocked | merged | superseded`
- `queue_block_reason` / `queue_blocked_at`
- `next_action`: `open_pr | resolve_conflict | combine_prs | await_merge`

`/v0/waves/{id}` reports:

- `open_pr_count`
- `stack_count`
- `has_stale_pr_state`

`LFD_GITHUB_TOKEN` enables live PR refresh during reconciliation and poll cycles.
