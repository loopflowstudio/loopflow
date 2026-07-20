---
requires: optional version input in message text (patch|minor|major|vX.Y.Z)
produces: RELEASE_NOTES.md, tagged release
diff_files: false
---
Run release as a one-shot operation that owns the full lifecycle.

## Input

`lf release-run <version>` passes `<version>` as message text. Interpret the
first token as:

- `patch` / `minor` / `major` (bump from latest tag)
- explicit version: `vX.Y.Z` or `X.Y.Z`

If no input is provided, default to `patch`.

## Workflow

Run exactly one command:

```bash
lf release run <version>
```

That command is responsible for:
- checking the exact target-scoped git range since the previous tag
- bumping manifests and generating release notes
- promoting any staged `release/unreleased/` artifacts to `release/v<version>/`
- archiving the generated root `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`
- creating and landing the release PR
- waiting for merge queue completion
- tagging the merged commit
- waiting for the target's configured completion evidence

The repository owns `verify` and `prepare` commands plus the selected workflow
under `release.targets` in `.lf/config.yaml`. Keep builds, signing, packaging,
migrations, registry uploads, deployments, smoke tests, and secret handling in
those repo-owned commands or workflows.

## Re-entry

`lf release run` resumes an existing release PR or incomplete latest tag after
interruptions.

## Guardrails

- The version comes from the human or wave config. Don't decide it.
- Be concrete in release notes: include real shipped changes, not vague summaries.
- If required data is missing (tags/gh auth/workflows), state exactly what command failed and what to run.

## Adaptation

If you discovered repo-specific release conventions — changelog format, tag
scheme, deploy hooks, version file locations — encode them in `.lf/config.yaml`
or repo-owned scripts. Copy this skill to `.lf/skills/release-run.md` only when
the release judgment itself needs to differ.
