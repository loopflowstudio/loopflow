# Release decisions ledger review

## What was implemented

Added a release decisions ledger under `release/unreleased/DECISIONS.md`, taught `lf op release run` to promote that directory into `release/v<version>/`, and archive the generated root `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`. Updated the release-notes prompt so it prefers the decisions ledger when present and falls back to merged PR history otherwise. Updated repo docs, release docs, and prompt goldens to match the new workflow.

## Key choices

- Use `release/unreleased/DECISIONS.md` as the narrative source of truth for a release cycle instead of deriving everything from merged PR titles.
- Promote `release/unreleased/` during the release workflow, not after tagging, so the generated notes and archived artifacts live under the target version before the release PR lands.
- Keep the decisions ledger optional. If it is absent or blank, release notes still generate from merged PR history.
- Archive versioned notes in `release/vX.Y.Z/NOTES.md` while leaving the repo-root `RELEASE_NOTES.md` as the always-latest view.

## How it fits together

`ops/release.rs` now loads `release/unreleased/DECISIONS.md`, promotes `release/unreleased/` into the version directory, passes the decisions text into the `release-notes` step, then copies the generated root notes into `release/v<version>/NOTES.md`. The built-in Loopflow/release prompts and repo docs now describe the same behavior, and the prompt goldens were refreshed so the prompt tests keep enforcing it.

## Risks and bottlenecks

- Prompt wording now changes the generated prompt snapshots, so future prompt edits need refreshed goldens.
- The decisions ledger is still manually maintained by interactive runs; this branch does not add a separate automation layer that recreates `release/unreleased/DECISIONS.md` after a release.
- Promotion still errors on filename collisions inside an existing `release/v<version>/` directory; the tests lock that behavior in.

## What's not included

- No automatic post-release recreation of `release/unreleased/DECISIONS.md`.
- No new UI around release artifacts.
- No change to release version selection, tagging rules, or GitHub release publication.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
