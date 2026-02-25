# Review: Move DMG to CI, simplify local install

## What was implemented

Deleted the 724-line `scripts/publish.py` and replaced it with two focused pieces:

1. **`scripts/install.py`** (277 lines) — local build and install only (`install.py local`). Extracted verbatim from `publish.py`'s `local` command and its helpers.

2. **`.github/workflows/release.yml` `build-dmg` job** — builds the Concerto DMG on macOS in CI and uploads it to Cloudflare R2 (versioned + latest). The DMG is also included as a GitHub Release artifact.

Additionally, `scripts/dev.py` gained a `screenshots` command that delegates to `generate_screenshots.py` (previously only accessible through `publish.py screenshots`).

## Key choices

**Delete publish.py entirely rather than slimming it.** The remote release flow (`patch`/`minor`/`major`/`recover`) was already replaced by `lf release patch` → merge → auto-tag → CI. The DMG build/upload moved to CI. Screenshots moved to `dev.py`. The only remaining value was `local`, which became `install.py`.

**Inline Python in CI for R2 upload.** The upload is ~15 lines and only runs in CI. A separate script would add a file that's never run locally. The inline approach keeps the logic visible next to the workflow step that needs it.

**No changes to the shell installer.** The `install.sh` heredoc in `release.yml` was already present and working. This branch only adds the DMG alongside it.

## How it fits together

```
Developer workflow:
  python scripts/install.py local [--service]   # local build+install

Release workflow (CI, triggered by v* tag):
  build-native  → tar.gz per platform
  build-dmg     → DMG + R2 upload
  publish-pypi  → PyPI
  publish-crates → crates.io
  release       → GitHub Release (tarballs + DMG + install.sh)
```

The `lf release patch` step writes `RELEASE_NOTES.md`, creates a PR, and on merge CI auto-tags, which triggers this workflow.

## Risks and bottlenecks

- **R2 credentials in CI.** The `build-dmg` job requires `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` as repository secrets. If these aren't set, the upload step fails.
- **`macos-latest` runner availability.** DMG build depends on a macOS runner. If GitHub Actions macOS runners are slow or unavailable, it could delay releases.
- **`pip3 install boto3` in CI.** This runs without pinning. A breaking boto3 release could fail the job. Low risk given boto3's stability, but worth noting.

## What's not included

- No migration of the `dmg` subcommand (manual DMG upload). That workflow is replaced by CI.
- No migration of `patch`/`minor`/`major`/`recover` subcommands. These were already replaced by `lf release patch` flow.
- No changes to the shell installer or native binary build.

## Fix applied during gate

- `install.py:218`: Changed stale "maturin" reference to "uv" in dry-run output (carried over from old publish.py).
