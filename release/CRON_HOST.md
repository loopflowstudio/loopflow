# Maintained cron Machine

Run the infrastructure Wave's telemetry and release jobs on the Machine that
runs the Wave's Tasks. That setting is the authority; a hostname in a document
is not.

```bash
scripts/bootstrap-cron-host.sh infrastructure
launchctl list | grep loopflow.cron.infrastructure   # macOS
crontab -l | grep loopflow:infrastructure            # Linux
```

With no Wave Machine default, the Wave runs where it is read, so any Machine
may bootstrap it. To pin it elsewhere, set your Machine for the Wave (a personal
value in this Machine's local store) and bootstrap from that Machine:

```bash
lf set machine mini --wave infrastructure
lf wave status infrastructure --json | jq -r '.wave.machine.id'   # must equal `lf machine id`
```

`bootstrap-cron-host.sh` fails before changing any schedule when the Wave is
placed on another Machine.

## Host prerequisites

- Promote an installed release `lf` (`lf self install`). Every check and both
  jobs run its entry gate, `~/.lf-machine/install/gates/1/lf`, whatever `lf`
  comes first on your shell's PATH; promotion replaces the binary behind it.
- Keep the repository's authoritative checkout on the placed Machine.
- Configure at least one managed Claude or Codex account in the Machine store.
- Install `uv`, `gh`, `cargo`, `flyctl`, `security`, `swift`, `xcrun`, and
  `jq` on the host path.
- Install an executable named `loopflow-release-publisher` on the Machine's PATH.
  It must inject the host's private publisher authority, then execute the argv
  it receives. Keep its implementation and provider selectors outside the
  checkout.
- Install the Developer ID signing identity and host-native GitHub, registry,
  R2, notarization, Sparkle (`SPARKLE_ED_PRIVATE_KEY`), and Fly authority required
  by the release publisher.

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

On macOS each job is a LaunchAgent in your login session's gui domain,
`~/Library/LaunchAgents/loopflow.cron.<wave>.<job>.plist`, with a daily
`StartCalendarInterval` (09:00 telemetry, 10:00 release). cron runs jobs outside
the login session, where the login Keychain holding provider and publisher
credentials is out of reach: the publisher finds no token and Claude logins
cannot be written. The agents need someone logged in to the Machine; the
bootstrap fails with that instruction when the gui domain is missing.

On Linux each job is a crontab line:

```
0 9 * * *  /bin/sh ~/src/loopflow/.lf/cron/infrastructure.telemetry-daily.sh # loopflow:infrastructure:telemetry-daily
0 10 * * * /bin/sh ~/src/loopflow/.lf/cron/infrastructure.release-run.sh # loopflow:infrastructure:release-run
```

Both schedulers run the same generated wrapper, which holds the ordinary lf
command in that minimal environment:

```bash
cat .lf/cron/infrastructure.release-run.sh
# cd ~/src/loopflow && exec env -i HOME=… PATH=… LF_HOME=… ~/.lf-machine/install/gates/1/lf --wave infrastructure --batch graph release-run >> .lf/logs/release-run.log 2>&1
```

cron reads only the first 999 bytes of a command and runs that prefix, and the
host PATH alone can exceed it, so the command lives in the wrapper. The
bootstrap refuses to write a line of 1,000 bytes or more.

The bootstrap's telemetry check runs the job the way its scheduler does: on
macOS `launchctl kickstart` starts the loaded agent and the bootstrap waits for
its exit code; on Linux it runs the wrapper. It never starts `release-run`.

`release-run` resolves to `.lf/graphs/release-run.yaml`, whose single operation
is `release run patch`. The release command's exit status is the job's; an
agent's successful report cannot turn a blocked release into success. Rerun the
bootstrap to rewrite the wrappers after the host path or checkout moves.

One scheduler runs each job. On macOS the bootstrap first removes the Wave's
`# loopflow:<wave>:` crontab lines, leaving other entries alone, then replaces
any loaded agent of the same label. macOS may ask permission for that crontab
write, so run the bootstrap from a logged-in Terminal; a bootstrap stopped at
the prompt has loaded no agent beside the old lines.

```bash
launchctl print gui/$(id -u)/loopflow.cron.infrastructure.release-run | grep -E 'state|last exit'
crontab -l | grep loopflow:infrastructure   # nothing on macOS
```

launchd runs a firing missed during sleep when the Machine wakes; cron skips
it. Both use host-local time. Place the Wave on an always-on Machine when
uninterrupted wall-clock cadence matters.

Loopflow remains the concrete deployment. Mirror this shape into Cadenza only
when its release needs it; do not extract a generic deployment platform.

## Release lock

The publisher launcher must preserve the inherited `LF_RELEASE_LOCK_FD`
descriptor and its environment reference through `process`. It is an OS lock
capability, not a permission flag. The Python publisher retains it through its
subprocesses, so a surviving publication child still excludes another release
when its parent exits. A launcher that closes it fails with a named diagnostic.
