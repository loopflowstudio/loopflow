---
requires: optional version input in message text (patch|minor|major|vX.Y.Z)
produces: RELEASE_NOTES.md, tagged release
diff_files: false
---
Run the full release workflow: check, bump, notes, commit, land, tag, verify.

## Input

`lf release <version>` passes `<version>` as message text. Interpret the first token as:

- `patch` / `minor` / `major` (bump from latest tag)
- explicit version: `vX.Y.Z` or `X.Y.Z`

If no input is provided, default to `patch`.

## Workflow

1. **Check for changes.** Skip if nothing merged since last tag.
   ```bash
   lf ops release-check
   ```
   Exit 1 means nothing merged — stop here.

2. **Resolve version.** If input is `patch`/`minor`/`major`, determine the next version from the latest tag. For the remaining steps, use the resolved version.

3. **Bump manifests.**
   ```bash
   lf ops release-bump <version>
   ```

4. **Generate release notes.** Analyze the merged PRs and write narrative notes.
   ```bash
   lf ops release-notes <version>
   ```

5. **Commit and land.**
   ```bash
   lf ops commit -m "release: v<version>"
   lf ops land
   ```

6. **Tag and push.**
   ```bash
   lf ops release-tag <version>
   ```

7. **Verify.** Confirm the release workflow passes.
   ```bash
   lf ops release-status
   ```
   If the workflow fails, report the failure and stop.

## Re-entry

Each command is idempotent. Before re-running phases:
- Check if the version commit already exists on main before re-bumping.
- Check if the tag already exists before re-tagging.

## Guardrails

- The version comes from the human or wave config. Don't decide it.
- Be concrete in release notes: include real shipped changes, not vague summaries.
- If required data is missing (tags/gh auth), state exactly what command failed and what to run.
