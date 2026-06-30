# Self-hosted deploy

Run your own `lfd` cron host. Use `deploy/PRIVATE_HOST.md` for the first maintained Tailscale host; use the generic Docker/Caddy path below for public or non-private hosts.

```bash
git clone https://github.com/loopflowstudio/loopflow.git /opt/loopflow
cd /opt/loopflow

export LF_DOMAIN='lfd.example.com'
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
export LF_TLS_MODE=internal   # Tailscale/private hosts; leave empty for public ACME

deploy/loopflow-server.sh up
```

Container mode and self-hosted bearer-token auth are the default. Keep secrets in Doppler; `.env` is the local fallback.

Cost guardrails live in `deploy/COSTS.md`. The maintained automation budget is $100/month; if actual or projected spend crosses it, stop before adding spend.

## Loopflow and Cadenza

Use the same server shape for both repos. The Terraform module and `loopflow-server.sh --repo PATH` are repo-parameterized; each repo should carry its own Docker/deploy files and Doppler project/config. Keep the cadence from `release/SCHEDULE.md` identical, then vary only product-specific build, smoke-test, signing, and publish commands.

## Secrets

```bash
doppler secrets set LFD_AUTH_TOKEN="$(openssl rand -hex 32)"
doppler secrets set GH_TOKEN=ghp_xxx
doppler secrets set ANTHROPIC_API_KEY=sk-ant-xxx
```

Minimum useful server secrets:

| Secret | What it does |
|--------|--------------|
| `LFD_AUTH_TOKEN` | Bearer token for remote API clients |
| `GH_TOKEN` | Lets agents push branches, inspect PRs, and react to CI |
| `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` / `GEMINI_API_KEY` | Agent provider credentials |
| `LFD_GITHUB_WEBHOOK_SECRET` | Optional webhook signature secret |

Use `docker/.env.example` only when running without Doppler.

## Operate

```bash
deploy/loopflow-server.sh up       # build and start
deploy/loopflow-server.sh update   # git pull default branch, rebuild, restart
deploy/loopflow-server.sh status
deploy/loopflow-server.sh logs
deploy/loopflow-server.sh health
```

The script uses Doppler automatically when the Doppler CLI is configured or `DOPPLER_TOKEN` is present. Run `doppler setup` in the repo, authenticate the host, or provide a Doppler service token outside the repo. Set `LOOPFLOW_SECRETS=env` to force plain Docker Compose.

Public hosts leave `LF_TLS_MODE` empty and use `deploy/Caddyfile`, letting Caddy request public ACME certificates. Private or Tailscale-only hosts set `LF_TLS_MODE=internal`; the deploy script mounts `deploy/Caddyfile.internal` for Caddy's internal CA.

## Bootstrap cron host

`bootstrap-cron-host.sh` is the one-command host setup. It installs the host service, starts `lfd`, waits for health, and creates or shows the root wave on the self-hosted daemon.

```bash
export LF_DOMAIN='lfd.example.com'
export LF_TLS_MODE=internal
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)

# Mac mini / Tailscale
deploy/bootstrap-cron-host.sh --host mac

# Linux host
deploy/bootstrap-cron-host.sh --host linux
```

Use Doppler for maintained hosts. On Linux the bootstrap writes `/etc/loopflow-server.env` with `0600` permissions so systemd can reach either `DOPPLER_TOKEN` or host-local fallback secrets. On macOS it writes the same launch environment into `~/Library/LaunchAgents/loopflow.server.plist`; prefer a Doppler service token over embedding long-lived provider credentials there.

The Linux update timer refreshes container hosts nightly. The native macOS private-host path installs `com.loopflow.lfd.update`, which refreshes the repo, rebuilds local binaries, and restarts `lfd` at 04:30 host-local time. Keep local dev binaries fresh with `scripts/pull-local-bin.sh`; server restarts should be predictable.

## Manual Linux service install

```bash
sudo install -m 0600 /dev/null /etc/loopflow-server.env
sudoedit /etc/loopflow-server.env   # DOPPLER_TOKEN=dp.st.x, LF_DOMAIN=..., LF_TLS_MODE=internal

# If the repo is not /opt/loopflow, edit WorkingDirectory and Exec* paths first.
sudo cp deploy/systemd/loopflow-server*.{service,timer} /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now loopflow-server.service
sudo systemctl enable --now loopflow-server-update.timer
```

## Manual Mac mini + Tailscale install

```bash
export LFD_HTTP_ADDR=0.0.0.0:2486
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
mkdir -p ~/.lf
printf '%s\n' "$LFD_AUTH_TOKEN" > ~/.lf/lfd-token
chmod 600 ~/.lf/lfd-token
deploy/native-lfd-host.sh install
```

The native install does not require Docker Desktop. It installs the `com.loopflow.lfd` service and `com.loopflow.lfd.update` nightly update agent. Both launchd jobs read the bearer token from `~/.lf/lfd-token` instead of embedding it in the plist.

Use the Docker Compose launchd path only when you explicitly want the container stack:

```bash
mkdir -p ~/Library/LaunchAgents
cp deploy/launchd/loopflow.server.plist ~/Library/LaunchAgents/
plutil -replace ProgramArguments.0 -string "$PWD/deploy/loopflow-server.sh" ~/Library/LaunchAgents/loopflow.server.plist
launchctl bootstrap "gui/$(id -u)" ~/Library/LaunchAgents/loopflow.server.plist
```

Use `LF_TLS_MODE=internal` for a Tailscale-only hostname; the deploy script mounts `deploy/Caddyfile.internal`. Leave `LF_TLS_MODE` empty for public ACME and it uses `deploy/Caddyfile`. Docker Desktop must be running.

## Enable repo crons manually

The bootstrap script does this when `lfq` is installed. To do it by hand from the host or a trusted client:

```bash
export LFD_URL=https://lfd.example.com
export LFD_TOKEN="$LFD_AUTH_TOKEN"
lfq create root /opt/loopflow
lfq show root
```

Wave YAML carries the cron schedule. For this repo, `wave/root/root.yaml` is the conductor cron; once the wave is created, `lfd` owns the scheduled runs.

## Verify

```bash
curl -f https://lfd.example.com/health
curl -H "Authorization: Bearer $LFD_AUTH_TOKEN" https://lfd.example.com/status
```

Point Concerto or `lfq` at the host and use the bearer token from `LFD_AUTH_TOKEN`. There is no studio discovery path; each repo owns its deployment.

## Troubleshoot

```bash
deploy/loopflow-server.sh status
deploy/loopflow-server.sh logs
curl -f http://127.0.0.1:${LFD_PORT:-2486}/health
```

Need agent credentials inside execution containers? Set `LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,codex,ssh` in Doppler or `.env` instead of editing compose volume lines.

Common failures:

- ACME cert not issued: DNS or port 80 is wrong
- WebSocket failures: make sure `/ws` is reachable through the same TLS host
- Remote clients cannot connect: verify `LFD_AUTH_TOKEN`, Caddy routing, and the client `Authorization: Bearer` header
- Agents cannot push: verify `GH_TOKEN` or mount `gh,ssh` credentials intentionally
