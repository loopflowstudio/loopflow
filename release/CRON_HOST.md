# Maintained cron Machine

Run the infrastructure Wave's telemetry and release jobs on your Wave Machine
for it. That setting is the authority; a hostname in a document is not.

```bash
lf machine use mini infrastructure
lf machine id
lf wave status infrastructure --json | jq -r '.wave.machine.id'
scripts/bootstrap-cron-host.sh infrastructure
crontab -l | grep loopflow:infrastructure
```

The two Machine ids must match. `bootstrap-cron-host.sh` fails before changing
the crontab when they differ.

## Host prerequisites

- Promote an installed release `lf`.
- Keep the repository's authoritative checkout on the placed Machine.
- Configure at least one managed Claude or Codex account in the Machine store.
- Install `uv`, `gh`, `cargo`, `flyctl`, `security`, `swift`, `xcrun`, and
  `jq` on the host path.
- Install an executable named `loopflow-release-publisher` on the Machine's PATH.
  It must inject the host's private publisher authority, then execute the argv
  it receives. Keep its implementation and provider selectors outside the
  checkout.
- Install the Developer ID signing identity and host-native GitHub, registry,
  R2, notarization, and Fly authority required by the release publisher.

The bootstrap reconstructs a minimal environment containing only host paths,
Machine/store paths, and basic locale/temp settings. It verifies a managed provider
account live, then runs the publisher's read-only check through:

```bash
loopflow-release-publisher \
  uv run python scripts/publish_release.py check
```

The logical command is the public contract; its Machine-local implementation owns
the private provider binding. The bootstrap never reads or prints a secret and
never forwards a Task lease, provider lease, GitHub token, PM token, or
invocation context. A missing command or failed check stops before the crontab
changes.

## Release schedules

Each job is an ordinary crontab line running an ordinary lf command in that
minimal environment:

```
0 9 * * *  cd ~/src/loopflow && env -i HOME=… PATH=… LF_HOME=… lf --wave infrastructure --batch graph telemetry-daily >> .lf/logs/telemetry-daily.log 2>&1
0 10 * * * cd ~/src/loopflow && env -i HOME=… PATH=… LF_HOME=… lf --wave infrastructure --batch graph release-run >> .lf/logs/release-run.log 2>&1
```

`release-run` resolves to `.lf/graphs/release-run.yaml`, whose single operation
is `release run patch`. The release command's exit status is the job's; an
agent's successful report cannot turn a blocked release into success. Rerun the
bootstrap to rewrite the lines after the host path or checkout moves.

Earlier lf scheduled these jobs as launchd agents named
`loopflow.cron.<wave>.<job>`. The bootstrap unloads and deletes them before
writing the crontab, so one scheduler runs each job. Check which one is live:

```bash
crontab -l | grep loopflow:infrastructure   # both jobs
launchctl list | grep loopflow.cron         # nothing
```

cron uses host-local time and skips firings while the Machine sleeps. Place the
Wave on an always-on Machine when uninterrupted wall-clock cadence matters.

Loopflow remains the concrete deployment. Mirror this shape into Cadenza only
when its release needs it; do not extract a generic deployment platform.

## Release lock

The publisher launcher must preserve the inherited `LF_RELEASE_LOCK_FD`
descriptor and its environment reference through `process`. It is an OS lock
capability, not a permission flag. The Python publisher retains it through its
subprocesses, so a surviving publication child still excludes another release
when its parent exits. A launcher that closes it fails with a named diagnostic.
