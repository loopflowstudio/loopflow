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
```

```bash
lf install                                   # install the latest published release
uv run python scripts/install.py local        # build only under local-bin/
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json # explicit disposable experiment
cat release/SCHEDULE.md      # hosted-build and cron-host release boundaries
```

`scripts/install.py local` builds this worktree's `lf`
and `Loopflow.app` into `<worktree>/local-bin/` with validation-only
migration authority. Local builds never change the installed CLI or main Home.
Use an explicit disposable `LF_HOME` for experiments.

Older installed CLIs still call `scripts/install.py refresh` after updating
main. That upgrade entry point delegates to `release/install.sh`, which verifies
the published artifacts and promotes them through the same transaction.

Published promotion stops before compilation while draft migrations remain.
Explicit experiments initialize the embedded schema in a fresh directory.
Promotion also snapshots the shared store, applies candidate migrations to the
copy, and expands every lifecycle reachable by placed open Work. An unresolved
flow or skill rejects the candidate before the installed binaries move.
On an active Home, promotion fences the old runtime generation, checkpoints and
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
maintained Home supplies that executable on PATH and keeps its credential
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
| Local | `lf install` | Download, verify, and promote the latest published control plane and Mac app | Yes, installed Home |

GitHub owns credential-free compilation. The maintained Loopflow host owns the
credentialed boundary: DMG signing/notarization, crates.io, R2, Fly deployment,
and the GitHub Release. It deploys the website from the exact tag and requires
`/healthz` to report that tag. If the proof fails it restores the previous Fly
image and leaves the release incomplete. Publishing the non-draft GitHub
Release is the final completion marker.

The publisher controller runs from current main while its source path is the
leased exact-tag worktree. This lets an incomplete immutable tag resume with a
release-plumbing repair without changing the code or artifacts being shipped.
An ambiguous Fly command result is accepted only when `/healthz` and the root
page prove the exact tag; rollback starts only after that production proof
fails.

The daily run is idempotent. No merged changes is success. If a valid tag's hosted
build succeeded but publishing stopped, the next run downloads that run's
artifacts and resumes the same tag. Cached receipts never bypass installer
preflight: preparation reuse and publication validate the packaged CLI in a fresh
disposable Home.

After integration, the publisher inspects the exact merged source before the
controller builds or tags it. A migration arriving after preparation causes a
new patch cut; canonical batches already on main stay immutable. For an invalid
tagged candidate, replacement first requires confirmed absence of a GitHub
Release (including drafts), crates.io version, and versioned R2 download.
Provider errors leave publication state unresolved. Partial publication needs
reconciliation; the controller never rewrites the old tag.

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
