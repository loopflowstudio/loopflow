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
daily schedules before it writes a plist. A declared `flow` must resolve to a
flow; missing or malformed repository content cannot fall back to a same-named
skill. Repository flows take precedence over reusable skills. Sync captures the
non-secret host path, Home id, Home/store paths, authoritative checkout, installed binary, exact
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

## Scheduled release settlements

```bash
lf release history --wave infrastructure --days 35 --json
lf cron disposition <failed-receipt-or-opportunity-id> \
  --wave infrastructure --owner <registered-task-work-id> \
  --reason "Repair the failed check; retry through the next configured firing"
```

Infrastructure resolves `release-run` to the repository's mechanical flow.
Each scheduled wake captures all outstanding original due times and executes
one release operation. Earlier misses point to that execution; they cannot
supply extra publication or no-change settlements. Due times arriving while
it runs wait for the next wake. An incomplete candidate keeps its original
owner, tag, and commit through recovery.
Collapse preserves each due time's first-attempt timing. Operator-triggered or
manual attempts on covered misses still disqualify the owner's unattended pair,
including when those misses fall before the displayed history window.

`release history` joins retained obligations, execution attempts, verification,
product outcomes, and physical cron receipts. It also reports failed telemetry
and late or missing repair dispositions. `observation_frontier: null` means
opportunity observation has not begun; historical process success never fills
that gap. Due times reconstructed before observation retain uncertain timezone
provenance. The report keeps older owner records when current rows refer to them.

The scheduler's exit status and the release outcome are separate facts. A zero
exit without product proof remains unverified. Published settlement requires
exact artifact hashes, required checks, and a public exact-version installer
smoke. No-change requires an empty fetched source range, a fully verified
published baseline, and current scheduled telemetry. Deferred work retains its
reason and continuation; failed attempts survive later recovery.

Each release attempt freezes `telemetry` observations for its covered due times.
Original prerequisite intervals retain their receipt ids; an unobserved schedule
or timezone remains explicitly unknown. Linked receipts stay in the history
response even when older than its requested window.

Missing or failed current telemetry gets at most one automatic retry through the
installed cron executor before release selection. Its receipt has source
`recovery` and is reserved on the attempt before launch. Catch-up does not retry
once per missed day. A running prerequisite defers release while its runner is
live or its identity is unknown. New receipts retain the runner's OS start time;
confirmed runner death permits recovery only after the existing job lock can be
acquired. A surviving check keeps that lock. Historical receipts without start
identity remain unresolved, regardless of age or PID availability. Recovery
retains the interrupted receipt without inventing its result; a failed retry
stops release. Recovery does not rewrite failures, supply repair ownership, or
change `doctor`'s scheduling-continuity checks. The existing target executor
remains synchronous; this limit bounds retry count, not target runtime.

A repair disposition references an existing registered Task and records local
repair ownership. It does not start work, assign a remote issue, or erase the
failure. The earliest disposition timestamp determines whether ownership was
recorded within a day. Pending external handoffs remain pending.

Ordinary `cron trigger` records intervention before asking launchd to kickstart;
it does not terminate an active job. A firing that may have resulted from that
request is marked `triggered`. Triggered executions cannot qualify as unattended
settlements. Two adjacent original due opportunities need two distinct automatic
executions, with at least one publication; collapsed rows cannot form that pair.

The publisher launcher must preserve the inherited `LF_RELEASE_LOCK_FD`
descriptor and its environment reference through `exec`. It is an OS lock
capability, not a permission flag. The Python publisher retains it through its
subprocesses, so a surviving publication child still excludes another release
when its parent exits. A launcher that closes it fails with a named diagnostic.
