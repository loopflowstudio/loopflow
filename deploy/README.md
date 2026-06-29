# Self-hosted deploy

Run your own `lfd` cron host behind Caddy TLS.

```bash
git clone https://github.com/loopflowstudio/loopflow.git /opt/loopflow
cd /opt/loopflow

export LF_DOMAIN='lfd.example.com'
export LFD_AUTH_TOKEN=$(openssl rand -hex 32)
export LF_TLS_MODE=internal   # Tailscale/private hosts; leave empty for public ACME

deploy/loopflow-server.sh up
```

Container mode and self-hosted bearer-token auth are the default. Keep secrets in Doppler; `.env` is the local fallback.

## Loopflow and Cadenza

Use the same server shape for both repos. The Terraform module and `loopflow-server.sh --repo PATH` are repo-parameterized; each repo should carry its own Docker/deploy files and Doppler project/config. Keep the cadence from `release/SCHEDULE.md` identical, then vary only product-specific build, smoke-test, signing, and publish commands.

## Secrets

```bash
doppler secrets set LFD_AUTH_MODE=local
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

## Linux host

```bash
sudo install -m 0600 /dev/null /etc/loopflow-server.env
sudoedit /etc/loopflow-server.env   # DOPPLER_TOKEN=dp.st.x, LF_DOMAIN=..., LF_TLS_MODE=internal

# If the repo is not /opt/loopflow, edit WorkingDirectory and Exec* paths first.
sudo cp deploy/systemd/loopflow-server*.{service,timer} /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now loopflow-server.service
sudo systemctl enable --now loopflow-server-update.timer
```

The update timer refreshes the server nightly. Keep local dev binaries fresh with `scripts/pull-local-bin.sh`; server restarts should be predictable.

## Mac mini + Tailscale

```bash
mkdir -p ~/Library/LaunchAgents
cp deploy/launchd/studio.loopflow.server.plist ~/Library/LaunchAgents/
plutil -replace ProgramArguments.0 -string "$PWD/deploy/loopflow-server.sh" ~/Library/LaunchAgents/studio.loopflow.server.plist
launchctl load ~/Library/LaunchAgents/studio.loopflow.server.plist
```

Use `LF_TLS_MODE=internal` for a Tailscale-only hostname; the deploy script mounts `deploy/Caddyfile.internal`. Leave `LF_TLS_MODE` empty for public ACME and it uses `deploy/Caddyfile`. Docker Desktop must be running; the launch agent keeps the stack up every five minutes.

## Enable repo crons

Create waves on the self-hosted daemon from the host or from a trusted client:

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

Point Concerto or `lfq` at the host and use the bearer token from `LFD_AUTH_TOKEN`. Studio discovery is optional, not the default path.

## Troubleshoot

```bash
deploy/loopflow-server.sh status
deploy/loopflow-server.sh logs
curl -f http://127.0.0.1:${LFD_PORT:-2486}/health
```

Need agent credentials inside execution containers? Set `LFD_EXECUTOR_CREDENTIALS_MOUNTS=claude,ssh` in Doppler or `.env` instead of editing compose volume lines.

Common failures:

- ACME cert not issued: DNS or port 80 is wrong
- WebSocket failures: make sure `/ws` is reachable through the same TLS host
- Remote clients cannot connect: verify `LFD_AUTH_TOKEN`, Caddy routing, and the client `Authorization: Bearer` header
- Agents cannot push: verify `GH_TOKEN` or mount `gh,ssh` credentials intentionally
