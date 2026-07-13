---
layout: default
title: lfd Daemon Reference
---

# lfd Daemon Reference

`lfd` runs the loopflow daemon: the HTTP read surface, session registry,
GitHub webhook ingress translated to `lf` execs, provider token refresh, and
worktree cleanup. It launches no agent work; each Wave resident and each
registry-backed Task Session own their provider execution.

## Run Native lfd

Use `lfd` locally or for single-user setups on macOS/Linux.

- Storage: sqlite
- Auth: local capability token
- Config: none required

```bash
lfd install
lfd serve
```

`lfd` writes the local session token to `~/.lf/session-token`. Local clients
read it automatically. This is a machine-local capability token, not a user
account system.

Remote identity and user auth, including OAuth-backed remote access, are M3
future work.

## Run the daemon

```bash
lfd serve
```

The default listen address is `127.0.0.1:2486`. Override it with `LFD_HTTP_ADDR`.

Remote HTTP is not the self-hosted operations path yet. Use SSH for remote
administration. For local-network experiments only, set the remote bind address
and bearer token when installing. `lfd install` persists selected `LFD_*`
environment variables into the service file, so the daemon survives restarts
without hand-editing launchd or systemd units.

```bash
export LFD_HTTP_ADDR=0.0.0.0:2486
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
lfd install --force
```

Service files that contain token-like values are written with owner-only permissions.

## Install

```bash
lfd install
lfd install --force
```

`--force` replaces the existing native service file.

## Uninstall

```bash
lfd uninstall
```

## Migrations

```bash
lfd migrate
lfd migrate --status
```

`lfd migrate` applies sqlite migrations for the local registry.

## Authentication transport

Send credentials in the `Authorization` header:

```bash
curl -H "Authorization: Bearer <token>" "$LFD_ADDR/status"
```

Local clients read the machine-local session token. If you deliberately bind a
non-loopback address for a local-network experiment, use a configured bearer
token. `lfd` rejects malformed authorization values before provider validation:
use `Bearer <token>` with a non-empty token (max 4096 bytes) and no embedded
whitespace or control characters.

`lfd` also rejects auth-like query parameters (`token`, `api_key`, `secret`, and similar) with `400 Bad Request`.

## Configuration reference

`lfd` reads `~/.lf/lfd.yaml` and then applies environment-variable overrides.

### Environment variables

Daemon settings:

```bash
LFD_HTTP_ADDR     # listen address (default 127.0.0.1:2486)
LFD_DB_PATH       # sqlite path override
```

Auth tuning within a shape:

```bash
LFD_AUTH_TOKEN    # bearer token for deliberate non-loopback experiments
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
auth:
  token: bundled-session-token # set from Doppler or env for non-loopback experiments

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

## Query waves

The daemon serves reads to Loopflow and `lf`. Examples below use `$LFD_ADDR`:

```bash
export LFD_ADDR=http://127.0.0.1:2486
curl -s -H "Authorization: Bearer $(cat ~/.lf/session-token)" "$LFD_ADDR/v0/waves"
curl -s -H "Authorization: Bearer $(cat ~/.lf/session-token)" "$LFD_ADDR/v0/waves/shipper"
```

Wave intent lives in `wave/<name>/GOAL.md`:

```markdown
---
---

## Objective

Keep the release path verifiably green.

## Measures

- **Key Results**: nightly verification stays green for fourteen days.
```

Run it with `lf serve shipper`.

## Browse the flow catalog

```bash
curl -s "$LFD_ADDR/v0/catalog?repo=$(pwd)" | jq '.result.flows[] | {name, category, source}'
curl -s "$LFD_ADDR/v0/catalog?repo=$(pwd)" | jq '.result.skills[] | select(.name=="gate")'
```

`/v0/catalog` returns the resolved flow + skill catalog that Loopflow uses for the **Flows** tab. Pass `repo=/path/to/repo` to merge builtin definitions with repo-local `.lf/flows/*.yaml` and `.lf/skills/*.md` overrides. Omit `repo` to inspect the builtin catalog only.

## Sessions API

List live sessions:

```bash
curl -s "$LFD_ADDR/v0/sessions?active_only=true" | jq '.data[] | {id, wave_id, skill, source, status, tmux_name}'
```

Create a palette session:

```bash
curl -s -X POST "$LFD_ADDR/v0/sessions" \
  -H "Content-Type: application/json" \
  -d "{
    \"wave_id\": \"lfdwave_abc\",
    \"flow\": \"ship-roadmap\",
    \"worktree\": \"$(pwd)\",
    \"agent\": \"codex\"
  }"
```

Attach to the tmux session:

```bash
curl -s -X POST "$LFD_ADDR/v0/sessions/<session_id>/attach"
```

Cancel the session:

```bash
curl -s -X POST "$LFD_ADDR/v0/sessions/<session_id>/cancel"
```

The attach endpoint marks the session attached and returns tmux connection
info. Use `tmux ls` and `tmux attach -r -t <name>` for read-only inspection;
steering goes through the wave thread so it remains journaled.

## GitHub webhooks

```bash
POST /v0/hooks/github
```

Set `github.webhook_secret` or `LFD_GITHUB_WEBHOOK_SECRET` before enabling the webhook. `lfd` verifies `X-Hub-Signature-256` and ignores unsigned requests.

Webhooks translate inward through the daemon. CI and main-push events ride the
agent bus and fold into the wave's thread attributed. Merged PRs complete their
owning durable Task Sessions.

- check_run failure → `lf radio pub --channel <wave> --from github "CI failed: …"` (the wave's loop decides how to fix)
- PR merged → mark the owning Task Session merged and notify its Wave
- push to main → `lf radio pub --channel <wave> --from github "main moved: …"` for each wave in the repo

The store accepts the notification even while the wave sleeps. No wave resolved → dropped.
