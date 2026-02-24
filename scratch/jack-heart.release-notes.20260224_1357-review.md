# Review: LLM-generated release notes pipeline

## What was implemented

- Added `lf ops release <version>` to automate release-note generation and release PR creation.
- Added release pipeline wiring:
  - `.github/workflows/auto-tag.yml` tags `main` commits when `RELEASE_NOTES.md` is merged.
  - `.github/workflows/release.yml` now uses `RELEASE_NOTES.md` as the GitHub release body.
- Added `ops::release` flow to:
  - find the latest tag,
  - collect merged PRs since that tag,
  - generate notes via the built-in `release_notes` prompt,
  - write `RELEASE_NOTES.md`,
  - create and auto-merge a `release/vX.Y.Z` PR.
- Updated the release-notes prompt to output raw markdown with `# v{version}` header and user-impact theming.
- Added integration tests for release behavior.

## Key choices

- **Clean-tree guard before release**: `lf ops release` now fails fast on dirty worktrees to avoid accidental branch resets or mixed commits.
- **Release branch base is always default branch remote**: release branches are now created from `origin/<default-branch>` (not current HEAD) so unrelated feature commits cannot leak into release PRs.
- **Single source of truth for release text**: `RELEASE_NOTES.md` is used for both auto-tag parsing and GitHub release body.

## How it fits together

`lf ops release vX.Y.Z` drives a single Rust workflow (`ops::release`) that creates `RELEASE_NOTES.md` and a release PR. After that PR merges to `main`, `auto-tag.yml` reads the version from the notes header and creates/pushes the git tag. The existing `release.yml` runs on the tag and publishes with `RELEASE_NOTES.md` as the release body.

## Risks and bottlenecks

- **`gh` query window**: PR collection is date-based from the previous tag date; same-day edge cases may include more PRs than intended.
- **Agent output quality**: release-note quality depends on model output; malformed markdown is partially mitigated by header normalization.
- **Repo settings dependency**: auto-merge behavior still requires GitHub auto-merge to be enabled.

## What's not included

- No automatic version bumping or semver inference.
- No fallback path for repos without tags.
- No additional filtering of PRs beyond merged state, base branch, and merged-date search.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
