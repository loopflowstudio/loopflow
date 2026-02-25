# EC2 Dogfood Lane

## Problem

The remote lfd stack (Docker Compose + Caddy + static token) has never been assembled and validated over a real WAN connection. All existing tests run against localhost. The Caddyfile is hardcoded to `localhost, 127.0.0.1` with self-signed TLS — it won't work on EC2 with a real domain. There's no smoke script that exercises lfd through a TLS reverse proxy, which means SSE and WebSocket behavior through Caddy is unvalidated.

We need one stable remote lane running end-to-end before expanding API surface or shipping studio auth. The team can't dogfood remote workflows until this works.

## Approach

Three deliverables in this repo. Provisioning (IaC, security groups, DNS) lives in `../studio`.

### 1. Production-ready Caddy configuration

Replace the localhost-only Caddyfile with a parameterized version that works for any domain:

```caddy
{$LF_DOMAIN:localhost} {
	tls {$LF_TLS_MODE:internal}

	@websocket {
		header Connection *Upgrade*
		header Upgrade websocket
	}
	reverse_proxy @websocket lfd:2486

	reverse_proxy lfd:2486 {
		flush_interval -1
	}
}
```

Key changes:
- `{$LF_DOMAIN}` — set to the EC2 hostname or domain. Caddy auto-provisions Let's Encrypt when this is a real domain.
- `{$LF_TLS_MODE}` — defaults to `internal` (self-signed) for dev. Omit or set empty for production ACME.
- `flush_interval -1` — disables response buffering so SSE events stream immediately instead of being batched by Caddy.
- WebSocket matcher — explicit upgrade handling to ensure Caddy doesn't interfere with the WS handshake.

Update `docker-compose.prod.yml` to pass domain config and expose port 80 (ACME HTTP challenge):

```yaml
services:
  caddy:
    environment:
      LF_DOMAIN: "${LF_DOMAIN:-localhost}"
      LF_TLS_MODE: "${LF_TLS_MODE:-internal}"
    ports:
      - "443:443"
      - "80:80"
```

### 2. Remote smoke script

One runnable script: `scripts/test_remote_smoke.py`. Takes a remote host URL and token as arguments. Exercises the full test loop from the wave item:

```
uv run python scripts/test_remote_smoke.py --url https://lfd.example.com --token <token>
```

Scenarios (building on existing `api_harness.py` and `wave_scenarios.py`):

| # | Scenario | What it validates |
|---|----------|-------------------|
| 1 | `GET /health` | lfd reachable through Caddy TLS |
| 2 | Wave CRUD | Create, list, get, update, delete via authenticated API |
| 3 | Auth rejection | Unauthenticated request returns 401 |
| 4 | SSE streaming | `GET /v0/sessions/{id}/events` delivers events through TLS proxy |
| 5 | WebSocket | `GET /ws` upgrade succeeds, receives `connected` message with waves snapshot |
| 6 | Wave run + logs | `POST /v0/waves/{id}/run`, then `GET /v0/waves/{id}/logs` streams output |
| 7 | Reconnect | Close WS, reopen, verify new connection receives current state |

The script reuses `ApiClient`, `ApiAssertions`, and `ScenarioRunner` from `scripts/lib/`. It adds a `WebSocketClient` wrapper for WS scenarios using the `websockets` library (already in the dev dependency tree via httpx).

What the script does NOT test:
- Fork execution (requires agent image + API keys on host — separate validation)
- Editor/terminal launch (requires SSH — manual verification)
- Concerto UI (manual — connect, browse, run a wave)

These are manual steps documented in `deploy/README.md`.

### 3. Deploy documentation

`deploy/README.md` captures the EC2 setup procedure end-to-end. Not IaC (that's studio), but the complete runbook for standing up the Docker Compose stack on a fresh EC2 instance.

Sections:
- **Prerequisites**: EC2 instance (Ubuntu 22.04+, t3.medium+), Docker + Compose, domain pointed at instance IP, ports 80/443 open
- **Quick start**: Clone, set env vars, `docker compose up`
- **Configuration**: LF_DOMAIN, LFD_AUTH_TOKEN, credential mounts, executor image
- **Verification**: Run smoke script
- **Credential mounts**: How to provide API keys for agent execution
- **Troubleshooting**: Common failure modes (TLS provisioning, WS timeouts, auth failures)
- **Manual test loop**: Concerto connection, editor launch, terminal access (the steps that can't be automated)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Full IaC (Terraform) in this repo | Reproducible provisioning | Wrong repo — provisioning belongs in `../studio`. This repo owns protocol and smoke validation. |
| Keep Caddyfile as-is, document workarounds | No code changes needed | Hardcoded localhost makes EC2 impossible. Self-signed certs break Concerto without trust dance. Real ACME is strictly better. |
| Shell-based smoke script | Simpler, fewer deps | Can't reuse existing Python test harness (ApiClient, ScenarioRunner). WebSocket testing is painful in bash. |
| Test SSE/WS separately from CRUD | Smaller, focused scripts | The whole point is validating the integrated path. One script, one run, one verdict. |

## Key decisions

**Caddy env vars over multiple Caddyfiles.** One Caddyfile with `{$LF_DOMAIN}` works for dev (localhost + internal TLS) and prod (real domain + ACME). No file switching, no template rendering.

**Python smoke script, not pytest.** The remote smoke is a validation tool, not a test suite. It runs against arbitrary remote hosts, not a hermetic fixture. `ScenarioRunner` gives pass/fail output without pytest overhead. Can be run from any laptop with `uv`.

**WebSocket scenario uses `websockets` library.** The `httpx` client doesn't support WebSocket. `websockets` is a lightweight, well-maintained library. Add as a dev dependency.

**SSE validation creates a real session.** No way to test SSE without a session that emits events. The smoke script creates a session, sends a trivial input, and verifies at least one event arrives within a timeout. This validates the full SSE path through Caddy without requiring a real agent.

**flush_interval -1 on Caddy reverse_proxy.** Without this, Caddy buffers SSE responses and events arrive in bursts instead of streaming. This is the most likely "it works locally but breaks remotely" issue.

**Manual test loop stays manual.** Concerto connection, editor launch, terminal access — these involve UI interaction and SSH. Document them as a checklist in `deploy/README.md`, don't try to automate.

## Scope

- **In scope**: Caddyfile parameterization, remote smoke script, deploy README, `websockets` dev dependency
- **Out of scope**: EC2 provisioning IaC (studio), JWT auth, new API endpoints, Concerto code changes, fork execution testing in smoke script

## Done when

```bash
# From laptop, against EC2:
uv run python scripts/test_remote_smoke.py --url https://lfd.example.com --token $TOKEN
# All 7 scenarios pass.
```

- Caddyfile works with real domain + ACME TLS (no self-signed cert dance)
- SSE events stream without buffering delay through Caddy
- WebSocket connects and receives state through Caddy
- `deploy/README.md` is sufficient to stand up a new EC2 instance without asking anyone
- Wave goal: *"EC2 lane can be reprovisioned from docs without tribal knowledge"* — the README is that doc
- Wave goal: *"SSE and WS survive Caddy TLS path under normal usage"* — the smoke script validates this on every run
