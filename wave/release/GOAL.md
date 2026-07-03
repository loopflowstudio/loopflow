---
primary_flow: build
pm:
  provider: asana
  asana_project: '1216257803789751'
---

Make releases boring and self-hosted: nightly verification that never deploys,
weekly publishing gated on that same verification, and a repo-owned `lfd`
running the crons — with Cadenza mirroring the cadence.

**Metrics to improve**
- Nightly builds/tests release artifacts and never deploys.
- Weekly publishes only after same-run verification passes.
- Self-hosted `lfd` runs repo crons from committed deploy files plus Doppler secrets.
- Secure remote execution via self-hosted bearer token; no hosted studio control plane.
- One command keeps local `lf`/`lfd`/app fresh.
- Loopflow and Cadenza keep carbon-copy release cadence.

**Milestones**
- Drain the buffer: keep local `lf`/`lfd`, release scripts, and CI aligned with merged infra.
- Cadenza release parity — same nightly/weekly cadence and updater.
- Bootstrap the first self-hosted cron host (Mac mini + Tailscale, Doppler).
- Close the feedback loop: failures become attention items or fix PRs.
