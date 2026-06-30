# Private cron host

Target one private host first. Keep it cheap: Tailscale private networking, bearer-token auth, Docker Compose, and Doppler/host env for secrets.

```bash
export LFD_HOST=<tailscale-ip-or-magicdns-name>
export LFD_SSH_USER=<ssh-user>
alias lfdhost="ssh $LFD_SSH_USER@$LFD_HOST"
```

## Current target

| Field | Value |
|-------|-------|
| Host | private Tailscale host or MagicDNS name |
| Tailscale address | host-local; do not commit specific values |
| SSH user | host-local; do not commit specific values |
| Client URL | `http://<tailscale-host-or-ip>:2486` |
| Auth | `Authorization: Bearer $LFD_AUTH_TOKEN` |

Use HTTP over Tailscale for the first cut. Tailscale provides the encrypted private network; `lfd` still requires the bearer token for non-loopback requests. This avoids Caddy internal-CA setup blocking `lfq`, Concerto, Codex, or Claude-driven sessions. Caddy/TLS can remain available for public or polished remote access later.

## Bring the host online

From this machine:

```bash
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale status
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale ping "$LFD_HOST"
ssh "$LFD_SSH_USER@$LFD_HOST"
```

If the CLI shim is stale, call the app binary directly. Local machines may have stale Tailscale shims after app renames or reinstalls.

## Bootstrap lfd on the host

On the host:

```bash
mkdir -p ~/src
git clone https://github.com/loopflowstudio/loopflow.git ~/src/loopflow
cd ~/src/loopflow

export LF_DOMAIN="$LFD_HOST"
export LF_TLS_MODE=internal
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
export LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh

deploy/bootstrap-cron-host.sh --host mac
```

Use Doppler for maintained secrets once the host is reachable:

```bash
doppler setup
doppler secrets set LFD_AUTH_TOKEN="$LFD_AUTH_TOKEN"
doppler secrets set LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh
```

The launch agent keeps the stack up. Docker Desktop must be running.

## Configure this Mac as a client

After `LFD_AUTH_TOKEN` exists on the host, run locally:

```bash
deploy/setup-private-client.sh --host "$LFD_HOST" --ssh-user "$LFD_SSH_USER" --token "$LFD_AUTH_TOKEN"
source ~/.lf/private-host.env
lfq list
```

The setup script writes:

- `~/.lf/private-host.env` with `LFD_HOST`, `LFD_SSH_USER`, `LFD_URL`, `LFD_TOKEN`, and an `lfdhost` SSH alias
- Concerto remote connection settings in `com.loopflow.concerto`
- Concerto token in Keychain service `loopflow.connection.token`, account `<host>:2486`

Open Concerto after running the script. It should connect to the host remote `lfd` and discover repos registered on that daemon.

## Remote sessions from local tools

Use the host as the execution host. Local clients only send control traffic.

```bash
source ~/.lf/private-host.env
lfq create root /Users/jack/src/loopflow
lfq show root
```

Concerto remote repo actions use the repo paths on the host. Remote terminal and IDE launches use SSH:

```bash
lfdhost
cursor --remote "ssh-remote+$LFD_SSH_USER@$LFD_HOST" /Users/jack/src/loopflow
code --remote "ssh-remote+$LFD_SSH_USER@$LFD_HOST" /Users/jack/src/loopflow
```

Claude and Codex sessions run inside `lfd` executor containers when waves or sessions use those harnesses. Keep the host's Doppler config or mounted credentials able to provide Claude, Codex, GitHub, and SSH credentials; do not route those through a global studio host.

## Verify

```bash
source ~/.lf/private-host.env
curl -f "$LFD_URL/health"
curl -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/status"
lfq list
uv run python scripts/test_remote_smoke.py --url "$LFD_URL" --token "$LFD_TOKEN" --repo /Users/jack/src/loopflow --insecure
```

`--insecure` is only for smoke scripts against a trusted Tailscale/private endpoint when TLS is involved. Plain Tailscale HTTP does not need it.

## Repair

On the host:

```bash
cd ~/src/loopflow
deploy/loopflow-server.sh status
deploy/loopflow-server.sh logs
deploy/loopflow-server.sh update
launchctl print gui/$(id -u)/loopflow.server
```

From this Mac:

```bash
source ~/.lf/private-host.env
lfq list
ssh "$LFD_SSH_USER@$LFD_HOST" 'cd ~/src/loopflow && deploy/loopflow-server.sh status'
```
