# Release artifacts

```bash
mkdir -p release/unreleased
$EDITOR release/unreleased/DECISIONS.md
lf release run patch
lf release notes 0.13.0 --preview  # inspect the cycle since v0.12.0
lf release run minor             # close the patch cycle, then publish its minor
find release -maxdepth 2 -type f | sort
```

```bash
scripts/bootstrap-cron-host.sh infrastructure  # preflight, sync, and configured-path receipts
lf cron history --wave infrastructure --days 35
lf release history --wave infrastructure --days 35 --json
```

```bash
lf install                                   # install the latest published release
uv run python scripts/install.py local        # build only under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json # explicit disposable experiment
cat release/SCHEDULE.md      # hosted-build and cron-host release boundaries
```

`scripts/install.py local` builds this worktree's `lf`
and `Loopflow.app` into `<worktree>/local-bin/` with validation-only
migration authority. Local builds never change the installed CLI or main Machine.
Use an explicit disposable `LF_HOME` for experiments.

Older installed CLIs still call `scripts/install.py refresh` after updating
main. That upgrade entry point delegates to `release/install.sh`, which verifies
the published artifacts and promotes them through the same transaction.

Published promotion stops before compilation while draft migrations remain.
Explicit experiments initialize the embedded schema in a fresh directory.
Promotion also snapshots the shared store, applies candidate migrations to the
copy, and expands every lifecycle reachable by placed open Work. An unresolved
flow or skill rejects the candidate before the installed binaries move.
On an active Machine, promotion fences the old runtime generation, checkpoints and
drains exact Wave/Project/Task containment, advances the store, restarts the
same keeper, then reconciles every enabled open Work onto a new Run. `lf install`
prints the terminal upgrade result directly; no manual zero-Run window is
required.

Use `release/` to keep the rationale and notes for each shipped version close
to the code.

`lf release run` owns the portable lifecycle: exact change evidence, version
intent, an isolated release PR, the tag, and observed completion. This
repository owns migration checks and preparation in `.lf/config.yaml`, plus
package builds, signing, notarization, uploads, deployment, smoke tests, and
secrets in its workflows and scripts.

Failed release-PR checks enter the same watched CI repair as `lf pr land`.
Release preparation still rebuilds version metadata when main advances.
Blocked repairs retain their checkout so `lf release run` can resume the work.
The scheduled `release-run` Flow executes the release operation directly;
its failed result cannot be hidden by a successful agent report.

The repository names the logical `loopflow-release-publisher` command. The
maintained Machine supplies that executable on PATH and keeps its credential
provider and selectors untracked. `lf release run` invokes its read-only
`check` before changing release state, so missing host authority fails closed.

| Path | What it does |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger for release-worthy intent and policy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived copy of that ledger for one shipped version |
| `release/vX.Y.Z/NOTES.md` | Archived copy of the release notes generated for that version |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

## Shipped artifacts

Every tagged release publishes `lf` and the **Loopflow desktop app** as peers —
the app is not a side artifact.

| Artifact | Where | Versioned by |
|----------|-------|--------------|
| `lf-<target>.tar.gz` | GitHub Release | `Cargo.toml` |
| `Loopflow-<version>.dmg`, `Loopflow-latest.dmg` | R2 `downloads/` + GitHub Release | tag |
| `SHA256SUMS` | GitHub Release | release artifact bytes |
| `loopflow` crate | crates.io | `Cargo.toml` |

`Loopflow.app`'s `CFBundleShortVersionString`/`CFBundleVersion` are stamped from
the release version at build time (`RELEASE_TAG`), so the app reports the same
version as `lf --version` — no separate manifest to bump or drift.

## Automation rhythm

| Cadence | Workflow | What it does | Ships? |
|---------|----------|--------------|--------|
| Nightly | `Packages (nightly)` | Builds every native `lf` tarball, extracts each package, and smoke-tests `--version` | No — artifacts expire after 14 days |
| Daily | Loopflow host `release-run` cron | Checks host credentials, opens and lands a patch release when commits landed, waits for hosted builds, then publishes and deploys | Yes |
| Tag | `Release build` | Builds and smoke-tests the four native tarballs on GitHub's target machines; stores workflow artifacts for the host publisher | No |
| Local | `scripts/install.py local` | Build validation-only `lf` and `Loopflow.app` into `local-bin/` | No |
| Local | `lf install` | Download, verify, and promote the latest published control plane and Mac app | Yes, installed Machine |

GitHub owns credential-free compilation. The maintained Loopflow host owns the
credentialed boundary: DMG signing/notarization, crates.io, R2, Fly deployment,
and the GitHub Release. It deploys the website from the exact tag and requires
`/healthz` to report that tag. If the proof fails it restores the previous Fly
image and leaves the release incomplete. Publishing the non-draft GitHub
Release records an external effect. Scheduled settlement follows public artifact
read-back and exact-version installer smoke, with every required check retained.
Public installer smoke requires a running Docker engine. It installs the pinned
release in a disposable Ubuntu 24.04 ARM64 container, copies only public inputs,
and checks the selected CLI bytes plus the installed gate's version/help/list.
No host Machine or credentials enter the container. Native macOS version/help smoke
and signing/notarization remain separate checks; Linux smoke does not prove Mac
app installation. Missing Docker or failed cleanup prevents verified settlement.
In history JSON, `attempts[].verification` holds those checks once; each attempt
saves its checks and product outcome together. `attempts[].telemetry` retains
original prerequisite associations with retained schedule/Machine segments and the
current execution's automatic retry, if needed. Successful recovery leaves the
earlier failure and its repair disposition visible. An interrupted telemetry runner can recover after its exact
process identity is confirmed gone and its surviving children release the cron
job lock. The original receipt remains unresolved; the new check supplies current
verification. Receipts without runner identity cannot authorize that recovery.

Closed, unfinished release owners remain visible beyond the history window in
`summary.closed_unsettled`. Record repair ownership with the printed
`lf cron disposition` command on the original Machine. This preserves the candidate,
attempts and late ownership evidence without claiming product settlement or
transferring execution to a new Machine. A replacement schedule on the same Machine
continues its predecessors on the next wake, freezing all outstanding dues in
one attempt. The saved candidate keeps its original owner. A Machine change breaks
that continuation chain; removed schedules have no future wake.

The publisher controller runs from current main while its source path is the
leased exact-tag worktree. This lets an incomplete immutable tag resume with a
release-plumbing repair without changing the code or artifacts being shipped.
An ambiguous Fly command result is accepted only when `/healthz` and the root
page prove the exact tag; rollback starts only after that production proof
fails.

The daily run is idempotent. No merged changes settles only after the published
baseline and current required verification pass. Selection uses fetched origin
without resetting caller commits, index, or working files. If a tag's hosted
build succeeded but publishing stopped, the next run downloads that run's
artifacts and resumes the same tag instead of cutting another patch.
Scheduled overlap deferrals retain the next due time from the firing
obligation's calendar and Machine. That observation reserves no execution; if the
obligation has closed, the continuation names its opportunity repair command
on the original Machine instead.
Preparation, publication, and public verification retain the publisher checkout
lease in their child processes. If the controller dies, ordinary checkout
removal remains blocked until those children exit; unrelated checkouts remain
independent.
If a publisher launcher exits while a descendant survives, controller cleanup
also preserves the checkout and reports its path for later cleanup.
Configured repository verification and preparation hooks retain both the target
lock and their checkout lease, including preparation during PR rebuild. A
surviving hook keeps cleanup and another release blocked until it exits.
Manifest lockfile updates (`cargo update --workspace` and `uv lock`) retain both
locks during preparation, so their surviving children also keep the checkout
available and competing releases deferred.
Notes generation passes both locks through its nested CLI to the provider.
Its exact JSON input remains in `.lf/prompts/` under a unique name, so a surviving
provider can finish after launcher failure without losing its checkout or input.
PR rebuild uses the same exact-source recovery checks as candidate preparation:
divergent local branches and dirty existing checkouts remain intact for repair.
If creation returns an unexpected HEAD, retain its checkout and branch for
inspection before retry; a creation hook may still own work there.
Source fetches, checkout creation, rebuild resets and cleanup retain their held
locks through surviving Git children. Source checkout creation stays local;
it never synchronizes or resets the caller's main checkout. Release preparation
owns the explicit commit and branch push.
Release PR creation, base/title/body edits, readiness and auto-merge commands
retain the target lock. During preparation they retain the checkout lease too,
so a surviving command keeps that checkout available until it exits. Preparation
stages, commits and pushes with both locks retained by Git children, then shared
PR finalization creates the review surface
with the release title and notes. Re-arming a dropped request retains the target
lock while waiting for merge.
CI repair keeps the same landing and reserved Session/Exec. Release-owned
repairs inherit both locks through a direct child launch, so a surviving repair
continues to exclude publication and checkout removal after controller death.
The release waits for that repair’s process evidence before continuing.
Task merge-request revocation and failed-finalization compensation carry the
same held locks. Revocation completes before a new head is pushed or durable
settlement intent is cleared; interrupted attempts retain that intent for retry.

Tag pushes, candidate-ref changes, workflow submissions, and GitHub publication
commands retain the release target's lock in their child process. If the
controller dies during one of these operations, another release invocation
defers until the surviving child exits. Other repositories and targets remain
independent.
Manual tag and publication commands record intervention on pending scheduled
releases. Nested publisher calls reusing the active lock preserve the owning
execution's provenance.

An invalid untagged candidate is corrected under the same version after exact-source
inspection confirms that preparation remains and GitHub, crates.io, and the
versioned DMG are all unpublished. The corrected PR uses a retry branch derived
from the rejected commit; earlier preparation commits remain in history.
Unknown or partial publication blocks replacement. Release history retains the
rejected candidate and inspection on the same opportunity's attempt.
A valid interrupted candidate resumes unchanged. Cached packaged binaries must
still pass candidate installation preflight before reuse or publication. Each
check runs the exact packaged ARM64 Linux CLI in a new Ubuntu 24.04 container,
with networking disabled and no host mounts or forwarded credentials. Preparation
uses the same check. Docker must be running; rejection or failed cleanup stops
preparation/publication. This proves fresh Linux-account preflight, not macOS
installation; native macOS packaging and smoke checks remain separate.

Required headless Desktop checks remain in gate and CI; the optional UI-host
exercise is not a publication prerequisite.
The publisher retains candidate hashes and gate evidence before external writes.
Its `verify --tag <tag>` mode checks the public asset set and hashes, both versioned
and latest DMGs, website release identity, crate version, and installed `lf`
version and selected bytes in a disposable Linux container. Scheduled settlement
uses `reconcile --tag <tag>`:
it repairs missing crate/versioned-DMG publication and stale website/latest-DMG
stages from the exact source and verified public artifacts, then repeats read-back.
Repair requires this tag to remain GitHub's latest release. Unavailable services
and conflicting immutable bytes fail without overwriting them. A crash after
publication does not require signing again or republishing GitHub assets just to
obtain a receipt. Public assets do not substitute for required verification.

After integration, the publisher inspects the exact merged source before the
controller builds or tags it. A migration arriving after preparation causes a
corrected cut at the same version, appending a canonical migration batch while
preserving earlier batches. A failed build or publisher preparation likewise
keeps the version. Tagged releases must finish or report their recovery blocker;
the controller neither rewrites their tags nor skips ahead to another version.

The configured publisher's read-only `inspect --commit SHA --tag TAG` emits JSON
with `preparation_required` (a list of reasons) and `publications` (null when
unchecked). `--check-publication` queries external publication when source needs
preparation; only a successfully queried empty list permits replacement.
Progress goes to stderr. The publisher owns repository-specific source and
publication facts; the release controller owns version selection and retries.
The runner leases that tag's publisher worktree until the publisher exits, so
concurrent re-entry and worktree cleanup cannot remove a checkout still in use.

Minor releases summarize a completed patch cycle. When changes remain,
`lf release run minor` publishes the next patch first. When the latest completed
patch already contains everything, it reuses that patch. The minor uses the
same product snapshot; version metadata and release notes change. Patch notes
compare against the preceding release, while minor notes compare against the
preceding `.0` tag. A missing cycle baseline is reported explicitly.
An explicit minor version such as `lf release run 0.13.0` follows the same policy.

The selected pair and snapshot survive interruption in
`.lf/releases/minor-<target>.json`. Retrying reuses a valid closing patch; replacing
an unprepared candidate advances the closing patch within the same cycle. A
completed successor is recovered even if publication finished before the pair
was saved. A minor candidate whose merged tree differs from its
prepared snapshot is stopped before tagging. Notes previews print Markdown to
stdout and progress to stderr, without changing manifests or release archives.
For an existing version, previews end at its tag and read historical release
context. Unreleased versions end at HEAD.

Append to `release/unreleased/DECISIONS.md` only when the change captures durable intent: policy choices, scope calls, paths not taken, or decisions a contributor would cite months later. Skip bug-fix churn and mechanical edits.

Interactive runs may append those decisions as they happen. Headless runs do
not. If `release/unreleased/` exists, `lf release run` promotes it to
`release/v<version>/`, uses `DECISIONS.md` to shape the narrative notes, and
writes the final notes to both `RELEASE_NOTES.md` and
`release/v<version>/NOTES.md`. The exact first-parent commit range is always
the shipped-behavior ledger; matching PRs add narrative context. If the
decisions directory is absent, the same commit evidence still produces notes.

Scheduled releases prefer the same agent-backed `release-notes` skill. Missing
CLIs, provider cooldowns, rate limits, quota exhaustion, authentication loss,
and provider outages select concise deterministic notes instead of stranding a
verified patch release. Release-note source context is capped at 128 KiB and
the resulting notes/merge-queue body at 60 KiB; omission counts travel with the
agent context. Unknown skill failures, stale-version output, missing output,
and oversized notes still block the release gate.

`lf release status` reports narrative notes, degraded-but-safe deterministic
notes, missing unsafe notes, or an unmarked legacy archive separately from the
workflow and GitHub Release status.
