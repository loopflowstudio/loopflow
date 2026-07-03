# release wave memory

Steers Loopflow toward boring releases: nightly verification that never deploys, weekly publishing gated on that same verification, and a repo-owned self-hosted `lfd` running the crons — with Cadenza mirroring the cadence.

## Shipped

- **Install syncs skills** — both install paths (refresh and local `--use`) run `lf op sync-skills --global --yes` after installing `lf`/`lfd`, so `~/.claude/skills` and `~/.agents/skills` always track the freshly installed binary. Sync failure warns but never fails the install; the binaries are already in place. First increment of "one command keeps local fresh."

## Model (design settled)

- Self-hosting is the default. The public repo carries containers, deploy scripts, service units, schedules, and docs; secrets live in Doppler or host-local env, never git.
- Nightly verifies release-grade artifacts with no publish or deploy side effects; weekly publishes only after equivalent verification passes in the same run.
- Secure remote execution binds remote `lfd` with `LFD_AUTH_TOKEN` behind TLS/Caddy/Tailscale. Studio auth and hosted discovery are gone.
- Loopflow carries the primitives; Cadenza mirrors the cadence and shape until a product-specific difference is deliberate and documented.
- Don't extract a generic multi-product deploy platform before a second or third real deployment proves the shape.
- Release owns the automation spine, not release-content substance: each product owns its own changelog and provider-specific agent credentials (beyond pass-through/secret wiring).

## Next

- **Drain current buffer** — keep local `lf`/`lfd`, release scripts, and CI aligned with the latest merged release-infra work. Known drift: `wave/*/items/*.md` and `wave/*/[0-9]-*.md` local roadmap mirrors survived the `asana-only` migration (c113ef04b) that was supposed to drop them — the roadmap now lives only in Asana (`lf op pm show`). Sweep these stale mirrors when a broader wave-hygiene pass runs; this update-wave run left them in place per the skill's "never delete local roadmap files" rule.
- **Cadenza release parity** (items/01) — same nightly/weekly cadence, one-command updater, tests, self-hosted assumptions; document any deliberate divergence.
- **Cron host bootstrap** (items/02) — bring up the first maintained self-hosted `lfd` host (Mac mini + Tailscale default), Doppler configured, root/conductor wave with scheduled checks.
- **Release feedback loop** (items/03) — failed nightly/weekly runs surface as attention items or focused fix PRs, distinguishing verification vs publish vs host vs stale-local drift.
- **Replicate intentionally** — apply the skeleton to Manabot/Hootro only when they need it.
