# Release notes pipeline (`lf ops release`)

`lf ops release vX.Y.Z` generates user-focused release notes, opens an auto-merging release PR, and lets CI create the tag and publish the GitHub release from `RELEASE_NOTES.md`.

## Current behavior

1. Verifies prerequisites (`gh` installed, clean working tree).
2. Resolves the previous tag with `git describe --tags --abbrev=0`.
3. Collects merged PRs since the previous tag date via `gh pr list`.
4. Uses the built-in `release_notes` prompt to generate markdown release notes.
5. Writes `RELEASE_NOTES.md` and ensures the header is `# v{version}`.
6. Creates `release/v{version}` from `origin/<default-branch>`, commits notes, pushes, opens PR (`release` label), and enables auto-merge.
7. After merge to `main`, `.github/workflows/auto-tag.yml` extracts the version from `RELEASE_NOTES.md` and pushes the tag.
8. `.github/workflows/release.yml` publishes the GitHub release using `body_path: RELEASE_NOTES.md`.

## Key decisions to keep

- **Single source of truth:** `RELEASE_NOTES.md` drives both auto-tagging and release body text.
- **Safe branch base:** release branch is created from `origin/<default-branch>` to prevent feature-branch commits leaking into release PRs.
- **Dirty-tree guard:** release flow exits early when there are uncommitted changes.
- **Explicit version:** no automatic semver bumping or inference.

## Known limitations

- PR collection is date-based from the previous tag; same-day merges may widen the inclusion window.
- No fallback path when no prior tag exists.
- Output quality depends on LLM response quality (header normalization is enforced, but content quality still varies).
- Auto-merge requires GitHub repository auto-merge settings to be enabled.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
