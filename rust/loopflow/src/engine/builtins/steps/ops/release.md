---
requires: optional version input in message text (patch|minor|major|vX.Y.Z)
produces: RELEASE_NOTES.md
diff_files: false
---
Generate release notes and update `RELEASE_NOTES.md`.

## Input

`lf release <version>` passes `<version>` as message text. Interpret the first token as:

- `patch` / `minor` / `major` (bump from latest tag)
- explicit version: `vX.Y.Z` or `X.Y.Z`

If no input is provided, default to `patch`.

## Workflow

1. Resolve target version from message text.
2. Find previous tag:
   ```bash
   git describe --tags --abbrev=0
   ```
3. Find merged PRs since that tag (main/default branch):
   ```bash
   base=$(git remote show origin | sed -n '/HEAD branch/s/.*: //p')
   gh pr list --state merged --base "$base" --json number,title,body --limit 200
   ```
4. Draft release notes grouped by user-facing impact.
5. Write `RELEASE_NOTES.md` with `# v<version>` as the top header.

## Guardrails

- Be concrete: include real shipped changes, not vague summaries.
- Keep it scannable: short sections and bullet points.
- If required data is missing (tags/gh auth), state exactly what command failed and what to run.
