# Maintained cron host

A maintained cron host runs `lf` on a schedule so health checks and patch
releases happen whether or not anyone is at a keyboard. The runtime is explicit:
**launchd invokes top-level `lf` commands** — no daemon, no resident wave
listener required.

## The host

| Host | Tailnet | Role |
|------|---------|------|
| `mini-heart` | `100.96.227.95` (`*.tail0eda02.ts.net`), macOS | First maintained `lf cron` host |

Reach it over Tailscale: `lf ssh mini-heart --remote-native -- lf --version`. First contact needs
the host key trusted and Tailscale up on both ends. `lf ssh` is bounded
(`ConnectTimeout=10`, `BatchMode=yes`), so an unreachable host fails in seconds
instead of hanging.

The host is a committed fact on purpose — an agent should discover it by reading
the repo, not by opening the Tailscale console.

## Prerequisites on the host

- `lf` on `PATH` (install/refresh via `scripts/install.py`).
- Doppler configured for the release project so secrets resolve without ever
  printing a value: `doppler setup` once, out of band. `lf cron sync` never reads
  or forwards secrets; scheduled `lf` runs resolve their own via Doppler.
- GitHub CLI auth able to create release PRs/tags, download workflow artifacts,
  and publish releases; an app-scoped Fly deploy token; crates.io, R2,
  notarization, and signing credentials available through Doppler.
- An Apple Silicon Mac with the Developer ID signing identity installed, plus
  `cargo`, `flyctl`, `gh`, `security`, `swift`, `uv`, and `xcrun` on `PATH`.
- An agent provider available for the `release` flow. Release-note generation
  has a deterministic fallback, but the flow itself is agent-owned.
- A checkout of this repo; run `lf cron sync` from inside it.

## Repo-owned schedules

Schedules live in the wave's `wave/<wave>/GOAL.md` frontmatter — one declaration,
read by both the resident wave listener and the launchd host:

```yaml
crons:
  - flow: telemetry-daily      # runs `op: doctor`; exits non-zero on any red check
    schedule: "0 0 9 * * *"    # 6-field cron expr: 09:00 daily
  - flow: release-run          # runs `lf release run patch`
    schedule: "0 0 10 * * *"   # after telemetry, host-local
```

Install / update them on the host in one idempotent command:

```bash
lf cron sync --wave infrastructure   # reconcile launchd jobs to match GOAL.md
lf cron list                         # prove what's installed
```

`sync` installs a launchd job per declared cron, prunes jobs for the wave whose
flow is no longer declared, and reports any schedule launchd can't run. Edit
`GOAL.md`, re-run `sync`, and launchd matches the declaration.

## Bootstrap

```bash
scripts/bootstrap-cron-host.sh mini-heart infrastructure
```

Probes reachability and host-native auth (bounded `lf ssh --remote-native`),
runs the release publisher's read-only credential/tool preflight, syncs the
repo-owned schedules, and lists the result. Idempotent and secret-free; re-run
it any time to reconcile.

## Failure surfacing (v0)

launchd writes each job's stdout+stderr to
`<repo>/.lf/logs/cron.<wave>.<flow>.log`; the release publisher additionally
writes redacted JSON receipts under `<repo>/.lf/logs/`. `lf doctor` (via
`telemetry-daily`) exits non-zero on a failed check. A red run leaves a non-zero
exit and a named stage in the log. No-change release runs exit successfully.

## Known limitations (v0)

- **Timezone.** launchd `StartCalendarInterval` fires on **host-local** time; the
  resident listener reads the same cron expression as UTC. The declared HH:MM is
  applied host-local on the launchd host. Fine for a daily health check; set the
  host clock/timezone deliberately.
- **Schedule shape.** Only a fixed daily time translates to launchd. Sub-daily,
  weekly, or multi-time expressions are reported as skipped by `sync` (the resident
  listener still honors the full expression). The launchd host is daily-only for now.

## Scope

Loopflow is the first real deployment. Cadenza mirrors this cadence as separate
follow-on work once this host proves the shape — do not generalize a multi-product
deploy layer before a second host needs it.
