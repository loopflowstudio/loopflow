# Maintained cron Home

Run the infrastructure Wave's declared telemetry and release jobs on the Home
that owns its durable placement. Placement is the authority; a hostname in a
document is not.

```bash
lf home id
lf status infrastructure --json | jq -r '.wave.home.id'
scripts/bootstrap-cron-host.sh infrastructure
lf cron history --wave infrastructure --days 35
```

The two Home ids must match. `bootstrap-cron-host.sh` fails before changing
launchd when they differ.

## Host prerequisites

- Promote an installed release `lf`; cron installation rejects development and
  task-worktree binaries.
- Keep the repository's authoritative checkout on the placed Home.
- Configure at least one managed Claude or Codex account in the Home store.
- Install `uv`, `gh`, `cargo`, `flyctl`, `security`, `swift`, `xcrun`, and
  `jq` on the host path.
- Install an executable named `loopflow-release-publisher` on the Home's PATH.
  It must inject the host's private publisher authority, then execute the argv
  it receives. Keep its implementation and provider selectors outside the
  checkout.
- Install the Developer ID signing identity and host-native GitHub, registry,
  R2, notarization, and Fly authority required by the release publisher.

The bootstrap reconstructs a minimal environment containing only host paths,
Home/store paths, and basic locale/temp settings. It verifies a managed provider
account live, then runs the publisher's read-only check through:

```bash
loopflow-release-publisher \
  uv run python scripts/publish_release.py check
```

The logical command is the public contract; its Home-local implementation owns
the private provider binding. The bootstrap never reads or prints a secret and
never forwards a Task lease, provider lease, GitHub token, PM token, or
invocation context. A missing command or failed check stops before cron sync.

## Repo-owned schedules

Schedules live in `wave/infrastructure/GOAL.md`:

```yaml
crons:
  - flow: telemetry-daily
    schedule: "0 0 9 * * *"
  - flow: release-run
    schedule: "0 0 10 * * *"
```

`lf cron sync --wave infrastructure` validates both targets and both fixed
daily schedules before it writes a plist. It captures the non-secret host path,
Home id, Home/store paths, authoritative checkout, installed binary, exact
schedule, and log path. Scheduled execution repeats the placement check and
fails with a receipt instead of running after ownership moves.

`lf cron preflight --wave infrastructure` performs the installed-binary,
placement, checkout, catalog, and schedule checks without changing launchd;
the bootstrap runs it before any credential probe.

launchd uses host-local time and coalesces missed calendar firings after wake.
Receipts record the actual start; declarations alone never count as evidence of
a nightly run. Place the Wave on an always-on Home when uninterrupted wall-clock
cadence matters.

## Durable evidence

```bash
lf cron list --wave infrastructure --json
lf cron trigger --wave infrastructure --flow telemetry-daily --wait --timeout 15m
lf cron trigger --wave infrastructure --flow release-run --wait --timeout 3h
lf cron history --wave infrastructure --days 35 --json
```

`list` reports the exact schedule, loaded state, installed Home, repo, binary,
and latest receipt. `trigger` exercises launchd rather than invoking the target
directly. Every firing writes a private, versioned receipt under
`<LF_HOME>/cron/receipts/<wave>/<flow>/`; receipts contain identity, timing,
outcome, exit status, and the log path, never environment values or output.

An early failure is `failed`, a successful no-op is `succeeded`, and a killed
runner remains `running` but is rendered `stale`. Detailed output stays at
`<repo>/.lf/logs/cron.<wave>.<flow>.log`. These rows begin the 14-night,
four-release, and 30-day authority/host-drift observation windows; bootstrap
does not claim that elapsed evidence in advance.

Loopflow remains the concrete deployment. Mirror this shape into Cadenza only
when its release needs it; do not extract a generic deployment platform.
