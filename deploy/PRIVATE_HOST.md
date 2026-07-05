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

Use HTTP over Tailscale for the first cut. Tailscale provides the encrypted private network; `lfd` still requires the bearer token for non-loopback requests. This avoids Caddy internal-CA setup blocking Concerto, Codex, or Claude-driven sessions. Caddy/TLS can remain available for public or polished remote access later.

## Bring the host online

From this machine:

```bash
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale status
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale ping "$LFD_HOST"
ssh "$LFD_SSH_USER@$LFD_HOST"
```

If the CLI shim is stale, call the app binary directly. Local machines may have stale Tailscale shims after app renames or reinstalls.

## Bootstrap native lfd on the host

On the host:

```bash
mkdir -p ~/src
git clone https://github.com/loopflowstudio/loopflow.git ~/src/loopflow
cd ~/src/loopflow

export LFD_HTTP_ADDR=0.0.0.0:2486
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
mkdir -p ~/.lf
printf '%s\n' "$LFD_AUTH_TOKEN" > ~/.lf/lfd-token
chmod 600 ~/.lf/lfd-token

deploy/native-lfd-host.sh install
```

Use Doppler for maintained secrets once the host is reachable:

```bash
doppler setup
doppler secrets set LFD_AUTH_TOKEN="$LFD_AUTH_TOKEN"
doppler secrets set LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh
```

The launch agent keeps native `lfd` up. A second `com.loopflow.lfd.update` launch agent runs `deploy/native-lfd-host.sh update` at 04:30 host-local time; that update path uses `scripts/install.py refresh` for the CLI rebuild. Both plists read the token from `~/.lf/lfd-token` through `LFD_AUTH_TOKEN_FILE`; the bearer token is not embedded in launchd config. Docker Desktop is not required for the native service. Use `deploy/bootstrap-cron-host.sh` only when you explicitly want the Docker Compose stack.

## Configure this Mac as a client

After `LFD_AUTH_TOKEN` exists on the host, run locally:

```bash
deploy/setup-private-client.sh --host "$LFD_HOST" --ssh-user "$LFD_SSH_USER" --token "$LFD_AUTH_TOKEN"
source ~/.lf/private-host.env
curl -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/v0/waves"
```

The setup script writes:

- `~/.lf/private-host.env` with `LFD_HOST`, `LFD_SSH_USER`, `LFD_URL`, `LFD_TOKEN`, and an `lfdhost` SSH alias
- Concerto remote connection settings in `com.loopflow.concerto`
- Concerto token in Keychain service `loopflow.connection.token`, account `<host>:2486`

Open Concerto after running the script. It should connect to the host remote `lfd` and discover repos registered on that daemon.

## Remote sessions from local tools

Use the host as the execution host. Local clients only send control traffic.

```bash
lfdhost
mkdir -p ~/src/loopflow/wave/root
echo "Drive this repo's roadmap." > ~/src/loopflow/wave/root/GOAL.md
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
curl -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/v0/waves"
uv run python scripts/test_remote_smoke.py --url "$LFD_URL" --token "$LFD_TOKEN" --insecure
```

`--insecure` is only for smoke scripts against a trusted Tailscale/private endpoint when TLS is involved. Plain Tailscale HTTP does not need it.

## Repair

On the host:

```bash
cd ~/src/loopflow
deploy/native-lfd-host.sh status
deploy/native-lfd-host.sh logs
deploy/native-lfd-host.sh update
deploy/native-lfd-host.sh install-update-agent
launchctl print gui/$(id -u)/com.loopflow.lfd
launchctl print gui/$(id -u)/com.loopflow.lfd.update
```

From this Mac:

```bash
source ~/.lf/private-host.env
curl -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/v0/waves"
ssh "$LFD_SSH_USER@$LFD_HOST" 'cd ~/src/loopflow && deploy/native-lfd-host.sh status'
```
