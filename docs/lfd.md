---
layout: default
title: lfd Daemon Reference
---

# lfd Daemon Reference

`lfd` runs the wave daemon, serving the HTTP API for wave management, agent sessions, and CI integration.

## Run the daemon

```bash
lfd serve
```

## Migrations

```bash
lfd migrate           # apply pending migrations for configured backend
lfd migrate --status  # print schema_version=<version>
```

`lfd migrate` uses the `mode` setting in `~/.lf/lfd.yaml` to choose backend (`sqlite` for native,
`postgres` for container). `LFD_DATABASE_URL` is required for postgres migrations.

## API examples

Examples below use `$LFD_ADDR`. Set it once per session:

```bash
export LFD_ADDR=http://127.0.0.1:2486  # default; pick a unique port per daemon
```

The default listen address is `127.0.0.1:2486`. Override with `LFD_HTTP_ADDR` when running multiple daemons.

## Authentication transport

Send credentials in the `Authorization` header:

```bash
curl -H "Authorization: Bearer <token>" $LFD_ADDR/status
```

`lfd` rejects malformed authorization values before provider validation. Use `Bearer <token>` with a non-empty token (max 4096 bytes) and no embedded whitespace/control characters.

`lfd` rejects auth-like query parameters (`token`, `api_key`, `secret`, etc.) with `400 Bad Request`.

## Install

```bash
lfd install          # install service for configured mode
lfd install --force  # tear down conflicting backend and reinstall
```

## Uninstall

```bash
lfd uninstall
```

## Query + manage waves

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

`repo_root` must point to a local repo containing `.lf/`, and `cwd` (when set) must resolve inside that repo root.

Stream events (replay + live tail):

```bash
curl -N "$LFD_ADDR/v0/sessions/<session_id>/events"
```

Session streams now include metering events:

- `context_snapshot` — emitted once at session start with prompt token composition (`sources`, `total`, `budget`, `diff_tier`)
- `turn_usage` — emitted after each `turn_completed` with per-turn token usage (`input_tokens`, `output_tokens`, optional provider-specific fields)

Supported harnesses: `codex`, `claude`, `opencode`. Session item payloads use normalized item types:
`command`, `file`, `message`, `thought`, and generic `tool`.

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

## Configuration

Environment variables:

```
LFD_MODE          # optional mode override: native or container
LFD_HTTP_ADDR     # daemon listen address (default 127.0.0.1:2486)
LFD_DB_PATH       # sqlite path override (relative to ~/.lf or absolute path)
LFD_DATABASE_URL  # required for container mode (postgres)
LFD_MAX_SLOTS     # concurrent run slots
LFD_AUTH_MODE     # local (default) or studio
LFD_AUTH_TOKEN    # optional session-token override for embedded/bundled launches
LFD_EXECUTOR_IMAGE # override agent image (default loopflow/agent:latest)
LFD_EXECUTOR_AGENT_TIMEOUT # override per-agent timeout (default 45m)
LFD_EXECUTOR_LIMITS_MEMORY # override docker memory bytes (default 8589934592)
LFD_EXECUTOR_LIMITS_MEMORY_SWAP # override docker memory+swap bytes (default 8589934592)
LFD_EXECUTOR_LIMITS_CPU_QUOTA # override docker CPU quota (default 400000 = 4 vCPU)
LFD_EXECUTOR_LIMITS_PIDS_LIMIT # override docker PID limit (default 1024)
LFD_GITHUB_WEBHOOK_SECRET  # required for /v0/hooks/github signature verification
LFD_GITHUB_TOKEN           # optional; enables startup/on-demand CI polling
LFD_HTTP_MAX_JSON_BODY_BYTES      # max body size for /v0/*, /status, /ws handshake (default 1048576)
LFD_HTTP_MAX_HOOK_BODY_BYTES      # max body size for /hooks/git and /v0/hooks/github (default 262144)
LFD_HTTP_MAX_WS_FRAME_BYTES       # max websocket frame size (default 65536)
LFD_HTTP_MAX_WS_MESSAGE_BYTES     # max websocket message size (default 262144)
LFD_HTTP_MAX_WS_QUEUE             # max per-connection outbound WS queue depth (default 256)
LFD_HTTP_MAX_WS_MALFORMED         # malformed WS messages allowed before disconnect (default 3)
LFD_HTTP_AUTH_FAILURES_PER_MINUTE # auth failures per (source, auth context, endpoint group) window (default 12)
LFD_HTTP_TRUSTED_PROXY_CIDRS      # comma-separated trusted proxy CIDRs for X-Forwarded-For parsing (default empty)
```

`lfd` reads `~/.lf/lfd.yaml` for daemon settings:

```yaml
mode: native  # native (default) or container

auth:
  mode: local # local (default) or studio
  token: bundled-session-token # optional; overrides the generated local session token
  base_url: https://auth.loopflow.studio # used by studio mode
executor:
  sandbox: false # default in container mode; set true to opt into experimental sandbox executor
  image: loopflow/agent:latest # base image for generated .lf/Dockerfile
  agent_timeout: 45m # max runtime per agent step (process or container)
  limits:
    memory: 8589934592 # 8 GiB
    memory_swap: 8589934592 # no swap above memory
    cpu_quota: 400000 # 4 vCPU
    pids_limit: 1024
  credentials:
    env: ["ANTHROPIC_API_KEY", "CODEX_API_KEY"]
    mounts:
      - claude
      - codex
      - ssh
github:
  webhook_secret: your-webhook-secret
  token: ghp_xxx # optional, used for startup /check-ci polling
http_security:
  max_json_body_bytes: 1048576
  max_hook_body_bytes: 262144
  max_ws_frame_bytes: 65536
  max_ws_message_bytes: 262144
  max_ws_queue: 256
  max_ws_malformed: 3
  auth_failures_per_minute: 12
  trusted_proxy_cidrs: [] # e.g. ["127.0.0.1/32", "10.0.0.0/8"]
```

`mode` (or `LFD_MODE`) selects a strict profile — `executor.type`, `storage`, `runtime_backend`, and
`service_manager` are all determined by the mode and cannot be overridden.

`credentials.mounts` uses named allowlisted mounts only:

- `claude` → `~/.claude`
- `codex` → `~/.codex`
- `gemini` → `~/.config/gemini`
- `gitconfig` → `~/.gitconfig`
- `ssh` → `~/.ssh`
- `gnupg` → `~/.gnupg`

Names are mounted read-only into the container. Raw `host:container` mount strings are rejected.

### Compose overrides

`lfd install` generates `~/.lf/docker-compose.yml` — don't edit it directly, it's regenerated on every install.

To customize the compose stack, create `~/.lf/docker-compose.override.yml`:

```yaml
# ~/.lf/docker-compose.override.yml
services:
  lfd:
    ports:
      - "3000:2486"   # expose on a different host port
    environment:
      - EXTRA_VAR=value
  postgres:
    command: postgres -c log_statement=all
```

Standard Docker Compose merge rules apply — the override file is layered on top of the managed file. `lfd` passes both files via `-f` flags when the override exists.

Environment overrides (non-identity fields only):

```bash
LFD_AUTH_MODE=studio
LFD_AUTH_TOKEN=bundled-session-token
LFD_EXECUTOR_IMAGE=loopflow/agent:latest # base image for generated Dockerfiles
LFD_GITHUB_WEBHOOK_SECRET=your-webhook-secret
LFD_GITHUB_TOKEN=ghp_xxx
```

Auth behavior:

- `auth.mode=local` (default) writes a startup session token to `~/.lf/session-token` (`0600` on Unix).
- All protected routes require `Authorization: Bearer <token>`.
- Clients (`lfq`, Concerto) auto-discover the local session token from `~/.lf/session-token`. Set `LFD_TOKEN` for shell use.
- `auth.token` / `LFD_AUTH_TOKEN` optionally override that local session token for embedded or bundled launches.
- In `auth.mode=studio`, `lfd` still writes a local session token for loopback clients; remote clients use connection tokens distributed by studio.

When `executor.type` is `docker`, `lfd` runs steps from a persistent Docker volume per repo (not a host bind mount). Each run uses a shared clone plus per-wave worktrees inside the volume and applies hygiene before execution (`git fetch`, `git reset --hard`, `git clean -fdx`).

Docker mode also:

- builds a repo-specific image tag (`lfd-agent-<repo-key>:latest`) from `.lf/Dockerfile`
- runs `install-loopflow.sh --install` in generated Dockerfiles when available in the base image
- treats `.lf/env-setup.sh` as project-owned setup; call `install-loopflow.sh "$@"` first in that script to keep loopflow base tooling aligned
- requires the `docker` CLI in `PATH` for repo image builds (`docker build`)
- reattaches to running agent containers after daemon restart
- enforces default container hardening (`user=agent`, memory/CPU/PID limits, `no-new-privileges`)

Fork steps are supported in Docker mode. Branch worktrees are created inside the repo volume, then cleaned up after synthesize and run completion.

## GitHub CI auto-fix

```bash
# Webhook target (GitHub check_run failures)
POST /v0/hooks/github

# One-shot poll for a single wave (requires github.token / LFD_GITHUB_TOKEN)
POST /v0/waves/{wave_id}/check-ci
```

Set `github.webhook_secret` (or `LFD_GITHUB_WEBHOOK_SECRET`) before enabling the webhook. `lfd` verifies `X-Hub-Signature-256` and ignores unsigned requests.

CI fix agents run the built-in `ci-fix` step (not `debug`) so prompts are scoped to resolving failing PR checks.

## Activation telemetry

```bash
# Recent activation decisions for a wave (queued/coalesced/dropped/dispatched)
GET /v0/waves/{wave_id}/activations?limit=50
```

Activation sources are `poll`, `push`, `listen`, or `manual`.

WebSocket streams also emit activation queue events:

- `activation_queued`
- `activation_coalesced`
- `activation_dropped`

## Stacked PR queue state

Wave PRs are created as Draft first. `lfd` reconciles queue roles so only the oldest unmerged run is promoted to Ready.

```bash
curl -s "$LFD_ADDR/v0/wave_runs?wave_id=<wave_id>&order=stack" | jq '.data[] | {id, stack_position, queue_role, queue_block_reason, next_action}'
```

`/v0/wave_runs` now includes:

- `queue_role`: `ready | draft | blocked | merged | superseded`
- `queue_block_reason` / `queue_blocked_at`
- `next_action`: `open_pr | resolve_conflict | combine_prs | await_merge`

`/v0/waves/{id}` reports:

- `open_pr_count` from live GitHub PR state (not historical snapshot state)
- `stack_count` for stack depth
- `has_stale_pr_state` when live PR state could not be refreshed

`LFD_GITHUB_TOKEN` enables live PR refresh during reconciliation and poll cycles.
