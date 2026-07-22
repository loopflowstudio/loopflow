# Release artifacts

```bash
mkdir -p release/unreleased
$EDITOR release/unreleased/DECISIONS.md
lf release run patch
find release -maxdepth 2 -type f | sort
```

```bash
lf cron sync --wave infrastructure   # daily patch release on the maintained host
lf cron list
```

```bash
uv run python scripts/install.py local --use   # full build: lf + Loopflow.app -> local-bin/, make active
uv run python scripts/install.py refresh       # update/install lf; unchanged main is a no-op
cat release/SCHEDULE.md      # hosted-build and cron-host release boundaries
```

`install.py` is the local entry point. `local --use` builds this worktree's
`lf` and `Loopflow.app` into `<worktree>/local-bin/`, then promotes that build.
`refresh` is the fast CLI-only path: pull the default branch, then rebuild and
install `lf` only when main moved or the install target is missing.

Promotion stops before compilation while draft migrations remain. Cut the
release first so the binary embeds the schema its runtime code expects.
Promotion also snapshots the shared store, applies candidate migrations to the
copy, and expands every lifecycle reachable by placed open Work. An unresolved
flow or skill rejects the candidate before the installed binaries move.

Use `release/` to keep the rationale and notes for each shipped version close
to the code.

`lf release run` owns the portable lifecycle: exact change evidence, version
intent, an isolated release PR, the tag, and observed completion. This
repository owns migration checks and preparation in `.lf/config.yaml`, plus
package builds, signing, notarization, uploads, deployment, smoke tests, and
secrets in its workflows and scripts.

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
| Local | `scripts/install.py local --use` | Build this worktree's `lf` and `Loopflow.app` into `local-bin/`, then promote it active | Local only |
| Local | `scripts/install.py refresh` | Pull, release-build `lf`, and atomically copy it into the local bin dir | Local only |

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

The daily run is idempotent. No merged changes is success. If a tag's hosted
build succeeded but publishing stopped, the next run downloads that run's
artifacts and resumes the same tag instead of cutting another patch.
The runner leases that tag's publisher worktree until the publisher exits, so
concurrent re-entry and worktree cleanup cannot remove a checkout still in use.

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
