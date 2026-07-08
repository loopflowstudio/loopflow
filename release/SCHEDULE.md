# Release schedule contract

Loopflow and Cadenza use the same release rhythm. Keep the cadence identical; only the repo-specific commands and package matrix should differ.

| Cadence | Time | Required gate | Publishes? | Repo-specific knobs |
|---------|------|---------------|------------|---------------------|
| Nightly package verification | `0 9 * * *` UTC | Build official packages, extract them, run smoke commands | No | package targets, build commands, smoke commands |
| Weekly auto-release | `0 12 * * 0` UTC | Run the nightly package verification first, then bump and tag only if commits landed since the last tag | Yes | manifests, release-note source, publish workflow |
| Local dev refresh | ad hoc / high frequency | Build from default branch and atomically replace local binaries/apps | Local only | install dir, artifact names |
| Self-hosted cron server refresh | nightly | Pull default branch and restart the server stack predictably | No | repo URL, host, domain, secrets project |

## Repository contract

Each repo should carry these files with carbon-copy cadence:

```text
.github/workflows/nightly-packages.yml   # same schedule; repo-specific package test body
.github/workflows/weekly-release.yml     # same schedule; calls nightly package verification before publishing
scripts/install.py                       # single local build entry: lf + lfd + desktop app -> per-worktree local-bin/
uv run python scripts/install.py refresh # or repo-equivalent CLI-only local updater
deploy/loopflow-server.sh                # or repo-equivalent self-hosted cron runner
deploy/systemd/* / deploy/launchd/*      # host keep-alive/update units
```

The desktop app is a first-class release artifact, not a side build. The
publishing workflow ships the signed app (`Loopflow.dmg`) next to the `lf`/`lfd`
tarballs, and the app's bundle version is stamped from the release tag so it
never drifts from the CLI. The single local build entry produces the CLI and the
app together into a per-worktree `local-bin/`.

Loopflow and Cadenza both carry the scheduled release workflow pair. Each repo replaces only the commands that are truly product-specific: Rust package targets for Loopflow; server image, Swift app build, signing-sensitive publish choices, and release manifests for Cadenza. Loopflow also carries the self-hosted cron-server shape because it is the automation host; product repos can copy that layer when they need repo-local services.

## Shared invariants

- Nightly jobs prove release artifacts without deploying them.
- Weekly publishing never runs unless nightly-style package verification passed in the same workflow run.
- Weekly publishing uses the canonical `lf op release run patch` path, including the `release-notes` skill and `release/unreleased/DECISIONS.md` narrative context. Do not duplicate release-note generation in workflow YAML.
- Release-note generation uses token compression: read the full release context, group repetition, preserve decisions and unique facts, and never substitute first-N commits or lines for summarization.
- Secrets stay in Doppler or host-local env files, never Terraform state or committed config.
- The cron server is self-hosted per repo. Studio discovery/auth is not supported.
- Local updater scripts refuse to pull a non-default branch unless explicitly told not to pull.
- Automation spend uses the company card feed as source of truth and stops for approval above $100/month.
