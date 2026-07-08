# Release artifacts

```bash
mkdir -p release/unreleased
$EDITOR release/unreleased/DECISIONS.md
lf op release run patch
find release -maxdepth 2 -type f | sort
```

```bash
uv run python scripts/install.py local --use   # full build: lf, lfd, Loopflow.app -> local-bin/, make active
uv run python scripts/install.py refresh       # CLI refresh: pull default branch, rebuild/install lf+lfd
cat release/SCHEDULE.md      # shared loopflow + cadenza release cadence
```

`install.py` is the local entry point. `local --use` builds this worktree's
`lf`, `lfd`, and `Loopflow.app` into `<worktree>/local-bin/`, then promotes that
build. `refresh` is the fast CLI-only path: pull the default branch, rebuild
`lf`/`lfd`, and install them into the local bin dir.

Use `release/` to keep the rationale and notes for each shipped version close to the code.

| Path | What it does |
|------|--------------|
| `release/unreleased/DECISIONS.md` | Append-only ledger for release-worthy intent and policy decisions during the current cycle |
| `release/vX.Y.Z/DECISIONS.md` | Archived copy of that ledger for one shipped version |
| `release/vX.Y.Z/NOTES.md` | Archived copy of the release notes generated for that version |
| `RELEASE_NOTES.md` | Always-latest release notes at the repo root |

## Shipped artifacts

Every tagged release publishes `lf`/`lfd` and the **Loopflow desktop app** as
peers — the app is not a side artifact.

| Artifact | Where | Versioned by |
|----------|-------|--------------|
| `lf-<target>.tar.gz` | GitHub Release | `Cargo.toml` |
| `Loopflow-<version>.dmg`, `Loopflow-latest.dmg` | R2 `downloads/` + GitHub Release | tag |
| `loopflow` wheel | PyPI | `pyproject.toml` |
| `loopflow` crate | crates.io | `Cargo.toml` |

`Loopflow.app`'s `CFBundleShortVersionString`/`CFBundleVersion` are stamped from
the release version at build time (`RELEASE_TAG`), so the app reports the same
version as `lf --version` — no separate manifest to bump or drift.

## Automation rhythm

| Cadence | Workflow | What it does | Ships? |
|---------|----------|--------------|--------|
| Nightly | `Packages (nightly)` | Runs the regression tier, builds every native `lf`/`lfd` tarball, extracts each package, and smoke-tests `--version` | No — artifacts expire after 14 days |
| Weekly | `Regression (weekly auto-release)` | Runs nightly package verification, bumps patch when commits landed since the last tag, tags, and dispatches `Release` | Yes |
| Release | `Release` (on tag) | Builds the native tarballs **and** the signed, notarized `Loopflow.dmg`; uploads the DMG to R2 and attaches it to the GitHub Release alongside `lf`/`lfd` | Yes |
| Local | `scripts/install.py local --use` | Build this worktree's `lf`, `lfd`, and `Loopflow.app` into `local-bin/`, then promote it active | Local only |
| Local | `scripts/install.py refresh` | `lf`/`lfd` only: pull, release-build, atomically copy into the local bin dir | Local only |

Nightly packages prove release artifacts while keeping deployment out of the loop. Weekly release reuses that package gate before publishing.

Append to `release/unreleased/DECISIONS.md` only when the change captures durable intent: policy choices, scope calls, paths not taken, or decisions a contributor would cite months later. Skip bug-fix churn and mechanical edits.

Interactive runs may append those decisions as they happen. Headless runs do not. If `release/unreleased/` exists, `lf op release run` promotes it to `release/v<version>/`, uses `DECISIONS.md` to shape the narrative notes, and writes the final notes to both `RELEASE_NOTES.md` and `release/v<version>/NOTES.md`. If the directory is absent, the workflow still runs and falls back to merged PR history.

Scheduled releases prefer the same agent-backed `release-notes` skill. When the runner has no Claude, Codex, or OpenCode CLI, `lf op release run` writes deterministic notes from the collected PRs and archived decisions instead of blocking the patch release.
