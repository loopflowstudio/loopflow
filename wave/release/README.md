# Release

Own the release and self-hosted automation spine for Loopflow first, then keep Cadenza in lockstep until the products truly need different machinery.

## Goal

Make releases boring: nightly verification proves packages without publishing, weekly release publishes only after verification, and repo-owned self-hosted `lfd` runs the crons. Secrets live in Doppler or host-local env. Deployment topology, scripts, schedules, and tests live in the repo.

This wave exists because release automation is now product infrastructure, not a private studio deployment. Loopflow should carry the primitives; Cadenza should mirror the cadence and shape; other product repos can copy the pattern when they need cron-backed automation.

## Success metrics

- **Daily verification:** nightly workflow builds/tests official release artifacts and never deploys them.
- **Weekly publishing:** weekly workflow publishes only after the same verification passes in that run.
- **Self-hosted execution:** a maintained `lfd` host can run repo crons from committed deploy files plus Doppler secrets.
- **No studio control plane:** secure remote execution uses self-hosted bearer-token auth; no hosted studio auth, registration, or daemon discovery path is required.
- **Local freshness:** developers have one clear script for pulling the latest local `lf`/`lfd` or product app into their bin/app location.
- **Parity:** Loopflow and Cadenza keep carbon-copy release schedules and infrastructure until a product-specific difference is deliberate and documented.

## Cadence

| Cadence | What happens | Gate |
|---------|--------------|------|
| Daily/nightly | Build packages, run release-grade tests, capture failures | No publishing or deploy side effects |
| Weekly | Publish release artifacts and notes | Nightly-equivalent verification passed in the same workflow |
| Continuous | Keep local tools fresh and self-hosted cron host healthy | One-command updater and status checks stay green |
| Per infra PR | Run a Mitchell Hashimoto simulated review | PR body includes tradeoffs and operational failure modes |

## Roadmap

1. **Drain current buffer** — keep local `lf`/`lfd`, release scripts, and CI aligned with the latest merged release-infra work.
2. **Cadenza parity** — mirror Loopflow's nightly/weekly cadence, local updater, tests, and self-hosted assumptions.
3. **Bootstrap the first cron host** — start with Mac mini + Tailscale unless AWS/Fly becomes simpler; configure Doppler; run repo-owned `lfd`; create the root/conductor wave.
4. **Close the feedback loop** — failed nightly/weekly jobs should surface as attention items or focused fix PRs, not disappear into Actions history.
5. **Replicate intentionally** — apply the same skeleton to Manabot/Hootro when they need it; don't abstract before the second or third real deployment proves the shape.

## Current operating decisions

- Self-hosting is the default. Public repo carries containers, deploy scripts, service units, schedules, and docs.
- Doppler hides secrets. Terraform, compose, Caddy, launchd/systemd, and scripts can be committed when they contain topology rather than credentials.
- Secure remote execution stays: bind remote `lfd` with `LFD_AUTH_TOKEN`, TLS/Caddy/Tailscale around it, and explicit clients.
- Studio auth and hosted discovery are gone. If pairing needs polish later, add QR/import UX on top of self-hosted URL + token rather than a global server.
- Cadenza follows Loopflow's release cadence until divergence is worth writing down.

## Not here

- Product release content decisions — each product owns its changelog substance.
- Provider-specific agent credentials beyond pass-through and secret wiring.
- A generic multi-product deploy platform. Keep the repo-local shape boring before extracting anything.
