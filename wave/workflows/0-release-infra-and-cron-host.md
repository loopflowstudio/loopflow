# Release infra and cron host

**Finish line:** Loopflow and Cadenza share one boring release system: nightly package verification, weekly release, hotfix release, local binary refresh, and a maintained self-hosted Loopflow cron host. The cadence is identical across repos; only repo-specific commands differ.

## Goal

Make releases routine enough that the system can ship without a human reconstructing state from chat history.

Loopflow owns the primitives and the cron host. Cadenza proves the product-repo shape: Fly app runtime, Neon Postgres, Doppler secrets, cheap production, and explicit admin/debug tooling. The two repos should feel carbon-copy where cadence and operations matter, even when build matrices differ.

## Measures

- **Nightly package verification is green** — official artifacts build, install/extract, and pass smoke commands without deploying anywhere.
- **Weekly release is gated** — publishing only happens after the same package verification passes in that workflow run and commits exist since the last tag.
- **Local refresh is one command** — a developer can pull the latest default-branch `lf`/`lfd` into a local bin path without remembering build internals.
- **Cron host is reachable and observable** — a self-hosted `lfd` runs scheduled flows, exposes health/log/admin checks, and can be updated predictably.
- **Cost stays under the standing budget** — automation/runtime spend remains under $100/month; crossing that line is the human blocker.
- **Secrets stay out of source** — committed infra describes topology and mechanics; Doppler or host-local env carries credentials.

## Process and cadence

| Cadence | What runs | Publishes? | Evidence |
|---------|-----------|------------|----------|
| Nightly | Build official packages and run smoke checks | No | workflow run + artifact smoke log |
| Weekly | Nightly-style verification, then release/tag when there are commits | Yes | release notes + tag + package checks |
| Hotfix | Manual release path with the same verification gate | Yes | PR/incident note + tag |
| Local refresh | Pull default branch, build, atomically replace local binaries/apps | Local only | script output |
| Cron host refresh | Pull default branch, restart host services, run health checks | No | admin log |

Run a Mitchell Hashimoto-style simulated review for each unit of work before calling it done. Findings either get fixed immediately or recorded in PR notes.

## Roadmap

1. **Codify the operating contract** — keep `release/SCHEDULE.md`, wave metadata, and PR notes aligned with the shared cadence.
2. **Get local Loopflow current** — make `scripts/pull-local-bin.sh` the single path for refreshing local `lf`/`lfd` from default branch.
3. **Bring up the self-hosted cron host** — run `lfd` on a maintained private host, keep it awake/reachable, and expose health/log/admin commands without committing private host details.
4. **Schedule verification** — wire nightly package verification and weekly release workflows for Loopflow and Cadenza with matching cron semantics.
5. **Budget reporting** — produce a simple monthly spend view across Fly, AWS, and agent/tooling providers where APIs allow it; stop before exceeding $100/month.
6. **Remote execution** — support secure remote Loopflow execution against the self-hosted host, scoped to self-hosting rather than a global studio server.

## Boundaries

### In scope

- Loopflow `lfd` self-hosting primitives
- GitHub Actions release cadence
- Local binary/app updater scripts
- Doppler-backed deploy/admin scripts
- Cost guardrails and budget reporting
- Cadenza parity where it proves the shared release shape

### Out of scope

- A studio-operated default global server
- Publicly committed private hostnames, Tailscale IPs, service tokens, or personal machine details
- Provider abstractions that do not serve an immediate deployed path
- Replacing vendor IDE/chat surfaces

## Done when

- Loopflow and Cadenza both carry the shared nightly/weekly release workflow shape.
- Local `lf`/`lfd` refresh is one command and used by default.
- A self-hosted `lfd` runs scheduled flows from a maintained host with documented admin/debug commands.
- Release status and scheduled automation show up in the same wave/run vocabulary as garden/govern work.
- Cost reporting is boring enough to run monthly without spelunking dashboards.
