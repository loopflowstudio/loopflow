---
requires: optional version input in message text (patch|minor|major|vX.Y.Z)
produces: RELEASE_NOTES.md, tagged release
diff_files: false
---
Run release as a one-shot operation that owns the full lifecycle.

## Input

`lf release <version>` passes `<version>` as message text. Interpret the first token as:

- `patch` / `minor` / `major` (bump from latest tag)
- explicit version: `vX.Y.Z` or `X.Y.Z`

If no input is provided, default to `patch`.

## Workflow

Run exactly one command:

```bash
lf ops release run <version>
```

That command is responsible for:
- checking there are merged PRs since the previous tag
- bumping manifests and generating release notes
- creating and landing the release PR
- waiting for merge queue completion
- tagging the merged commit
- waiting for release workflow completion

## Re-entry

`lf ops release run` should be safe to re-run after interruptions.

## Guardrails

- The version comes from the human or wave config. Don't decide it.
- Be concrete in release notes: include real shipped changes, not vague summaries.
- If required data is missing (tags/gh auth/workflows), state exactly what command failed and what to run.

## Adaptation

If you discovered repo-specific release conventions — changelog format, tag scheme, deploy hooks, version file locations — encode them. Most belong in repo docs where all steps benefit. Copy this step to `.lf/steps/release.md` when the repo needs release to work differently — a changed workflow, or team preferences about how releases happen.
