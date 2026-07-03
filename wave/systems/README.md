# Systems

Keep the engineering outfit efficient. Systems owns the machinery *around* the
code — CI, releases, tooling, automation, stability, the self-hosted spine — so
the product waves can move without fighting their own toolchain.

Paired with **architecture**: Architecture shapes the code and makes the
codebase true; Systems shapes the operation and makes the outfit fast and
reliable. Sharp edges get sanded, manual rituals get automated, flaky loops get
fixed.

## North-star metrics

Four numbers decide whether Systems is winning:

| Metric | Winning looks like |
|--------|--------------------|
| **Billing** | Infra + agent spend has a budget and no surprises |
| **Prod uptime** | The self-hosted `lfd` host and services stay up |
| **Green on main** | The merge gate is trusted and rarely red |
| **Test time** | Local and GitHub test runs trend down, never up |

Supporting: releases stay boring (verified → scheduled → repo-owned), one-command
freshness for local and host tools, and failures surface as work rather than
disappearing into Actions history.

## Responsibilities

**Owns:**
- **CI health** — green, fast, trusted gates (`ci.yml` and the nightly/weekly tiers)
- **Release pipeline** — verify→publish cadence, artifacts, DMG/notarization, notes, version + tag
- **Developer experience** — the freshness path, install, local build loops; one command for everything
- **Automation** — dependabot, crons, triggers, feedback loops; anything done twice by hand
- **Stability** — flaky tests, timeouts, hangs, retries; the reliability of the machinery
- **Self-hosted spine** — the cron host, secure remote execution, secrets, cost guardrails
- **Cadenza parity** — mirror the cadence and shape until a divergence is deliberate

**Does not own:**
- Code architecture and simplification — that's **architecture**
- Product changelog *substance* — each product owns its story; Systems owns the machinery
- Feature work and product surfaces — concerto, website, workflows
- Provider agent credentials beyond pass-through and secret wiring
- A generic multi-product deploy platform — keep the shape boring before extracting

## Hot-spot coverage

The zones where the operation actually breaks:

**Pipeline** — `ci.yml`; nightly (`nightly-packages.yml`, `regression-daily.yml`,
`docker-e2e-nightly.yml`); weekly (`weekly-release.yml`, `release.yml`,
`auto-tag.yml`); DMG sign→notarize→staple→R2; notes (`DECISIONS.md` →
`release/vX/`, `RELEASE_NOTES.md`).

**Distribution & freshness** — `scripts/install.py`, `pull-local-bin.sh`,
`deploy/native-lfd-host.sh`; docker images (`docker/`, `docker-compose.prod.yml`);
`website-deploy.yml`.

**Self-hosting spine** — `deploy/` (bootstrap, `loopflow-server.sh`,
launchd/systemd, Caddy, Tailscale, terraform); Doppler secrets; remote security
(`LFD_AUTH_TOKEN`, TLS, `test_remote_smoke.py`); cost (`deploy/COSTS.md`,
`budget.json`).

**Automation hygiene** — `dependabot-auto.yml` + `.github/dependabot.yml`; the
failure→work feedback loop.

**Outward** — Cadenza release parity (cross-repo reach).

## Cadence

| Cadence | What happens | Gate |
|---------|--------------|------|
| Daily/nightly | Build packages, run release-grade tests, capture failures | No publishing or deploy side effects |
| Weekly | Publish release artifacts and notes | Nightly-equivalent verification passed in the same run |
| Continuous | Keep local tools fresh, cron host healthy, main green | One-command updater and status checks stay green |
| Per infra PR | Run a Mitchell Hashimoto simulated review | PR body includes tradeoffs and operational failure modes |

## Roadmap

1. **Drain the buffer** — keep local `lf`/`lfd`, release scripts, and CI aligned with the latest merged infra work.
2. **Cron host bootstrap** — bring up the first maintained self-hosted `lfd` cron host (Mac mini + Tailscale default), Doppler configured, root/conductor wave running scheduled checks, status/logs/update documented.
3. **Feedback loop** — failed nightly/weekly runs surface as attention items or focused fix PRs, distinguishing package-verify vs. publish vs. host vs. stale-local failure.
4. **Cadenza parity** — mirror Loopflow's nightly/weekly cadence, updater, and self-hosted assumptions; any difference written down.
5. **Replicate intentionally** — apply the skeleton to other product repos only when a second or third real deployment proves the shape.

## Operating decisions

- Self-hosting is the default. The public repo carries containers, deploy scripts, service units, schedules, and docs.
- Doppler hides secrets. Terraform, compose, Caddy, launchd/systemd, and scripts can be committed when they carry topology, not credentials.
- Secure remote execution: bind remote `lfd` with `LFD_AUTH_TOKEN`, TLS/Caddy/Tailscale around it, explicit clients. No hosted studio auth or discovery plane.
- Cadenza follows Loopflow's cadence until divergence is worth writing down.
- The self-hosting spine stays inside Systems for now — split it into its own wave only once a second real deployment proves that shape.
