# Release schedule contract

Loopflow separates hosted compilation from credentialed publishing. GitHub
proves target-specific binaries; the maintained Loopflow host decides and ships
releases.

| Cadence | Time | Required gate | Publishes? | Repo-specific knobs |
|---------|------|---------------|------------|---------------------|
| Nightly package verification | `0 9 * * *` UTC | Build official packages, extract them, run smoke commands | No | package targets, build commands, smoke commands |
| Daily patch release | `0 0 10 * * *` host-local | Host preflight, merged changes, successful tag build, exact-tag website health | Yes | manifests, release-note source, publisher command |
| Local dev refresh | ad hoc / high frequency | Build from default branch and atomically replace local binaries/apps | Local only | install dir, artifact names |

## Repository contract

Loopflow carries these release boundaries:

```text
.github/workflows/main-packages.yml      # per-green-main packages; signs the dev app
.github/workflows/release.yml            # tag-triggered, credential-free native matrix
release-run                              # built-in daily entry point: lf release run patch
scripts/publish_release.py               # credentialed publisher and completion receipt
scripts/deploy_website.py                # exact-tag deploy, proof, and rollback
scripts/install.py                       # local lf + desktop app build entry
```

The desktop app is a first-class release artifact. The host publisher ships the
signed app (`Loopflow.dmg`) next to the `lf` tarballs, and stamps the app bundle
from the release tag so it cannot drift from the CLI. The local build entry
produces the CLI and app together into a per-worktree `local-bin/`.

## Shared invariants

- Nightly jobs prove release artifacts without deploying them.
- GitHub workflows use no Doppler, registry, R2, or deployment credentials. Signing is the exception: `Packages (main)` and the notarization check read the Developer ID certificate and notary key from the `signing` environment, which only `main` can use.
- Daily publishing uses the canonical `lf release run patch` path, including the `release-notes` step and `release/unreleased/DECISIONS.md` narrative context. Do not duplicate release-note generation in workflow YAML.
- A release is complete only after the tagged website is healthy and the GitHub Release is non-draft.
- Failed website proof restores the previous image. The next daily run resumes an incomplete successful tag build.
- Release-note generation reads the full release context, groups repetition, preserves decisions and unique facts, and never substitutes first-N commits or lines for summarization.
- Secrets come from Doppler on the maintained host and never enter committed config. GitHub holds a copy of the signing secrets in the `signing` environment; the repository has no Actions secrets, every environment that holds one allows `main` only, and the daily release checks that ([Signing secrets](../.github/CI.md#signing-secrets)).
- Local updater scripts refuse to pull a non-default branch unless explicitly told not to pull.
- Automation spend uses the company card feed as source of truth and stops for approval above $100/month.
