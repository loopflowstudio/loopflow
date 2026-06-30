# Mac Mini cron host

Target the Mac Mini first. Keep it cheap: Tailscale private networking, bearer-token auth, Docker Compose, and Doppler/host env for secrets.

```bash
export MINI=100.96.227.95
alias mini="ssh jack@$MINI"
```

## Current target

| Field | Value |
|-------|-------|
| Host | `mini-heart` |
| Tailscale IP | `100.96.227.95` |
| SSH user | `jack` |
| Client URL | `http://100.96.227.95:2486` |
| Auth | `Authorization: Bearer $LFD_AUTH_TOKEN` |

Use HTTP over Tailscale for the first cut. Tailscale provides the encrypted private network; `lfd` still requires the bearer token for non-loopback requests. This avoids Caddy internal-CA setup blocking `lfq`, Concerto, Codex, or Claude-driven sessions. Caddy/TLS can remain available for public or polished remote access later.

## Bring the Mini online

From this machine:

```bash
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale status
/Applications/Tailscale\ 2.app/Contents/MacOS/tailscale ping 100.96.227.95
ssh jack@100.96.227.95
```

If the CLI shim is stale, call the app binary directly. The local `/usr/local/bin/tailscale` may point at `/Applications/Tailscale.app` while the installed app is `/Applications/Tailscale 2.app`.

## Bootstrap lfd on the Mini

On the Mini:

```bash
mkdir -p ~/src
git clone https://github.com/loopflowstudio/loopflow.git ~/src/loopflow
cd ~/src/loopflow

export LF_DOMAIN=100.96.227.95
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

After `LFD_AUTH_TOKEN` exists on the Mini, run locally:

```bash
deploy/setup-mini-client.sh --token "$LFD_AUTH_TOKEN"
source ~/.lf/mini.env
lfq status
```

The setup script writes:

- `~/.lf/mini.env` with `MINI`, `LFD_URL`, `LFD_TOKEN`, and the `mini` SSH alias
- Concerto remote connection settings in `com.loopflow.concerto`
- Concerto token in Keychain service `loopflow.connection.token`, account `100.96.227.95:2486`

Open Concerto after running the script. It should connect to the Mini remote `lfd` and discover repos registered on that daemon.

## Remote sessions from local tools

Use the Mini as the execution host. Local clients only send control traffic.

```bash
source ~/.lf/mini.env
lfq create root /Users/jack/src/loopflow
lfq show root
```

Concerto remote repo actions use the repo paths on the Mini. Remote terminal and IDE launches use SSH:

```bash
mini
cursor --remote ssh-remote+jack@100.96.227.95 /Users/jack/src/loopflow
code --remote ssh-remote+jack@100.96.227.95 /Users/jack/src/loopflow
```

Claude and Codex sessions run inside `lfd` executor containers when waves or sessions use those harnesses. Keep the Mini's Doppler config or mounted credentials able to provide Claude, Codex, GitHub, and SSH credentials; do not route those through a global studio host.

## Verify

```bash
source ~/.lf/mini.env
curl -f "$LFD_URL/health"
curl -H "Authorization: Bearer $LFD_TOKEN" "$LFD_URL/status"
lfq status
uv run python scripts/test_remote_smoke.py --url "$LFD_URL" --token "$LFD_TOKEN" --repo /Users/jack/src/loopflow --insecure
```

`--insecure` is only for smoke scripts against a trusted Tailscale/private endpoint when TLS is involved. Plain Tailscale HTTP does not need it.

## Repair

On the Mini:

```bash
cd ~/src/loopflow
deploy/loopflow-server.sh status
deploy/loopflow-server.sh logs
deploy/loopflow-server.sh update
launchctl print gui/$(id -u)/loopflow.server
```

From this Mac:

```bash
source ~/.lf/mini.env
lfq status
ssh jack@$MINI 'cd ~/src/loopflow && deploy/loopflow-server.sh status'
```
