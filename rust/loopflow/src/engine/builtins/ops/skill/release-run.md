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

`minor` completes a closing patch when unreleased changes exist, or reuses the
latest completed patch when it already contains the release snapshot. The minor
publishes that same product state with new version metadata and cycle notes
against the preceding `.0` tag. Patch notes retain their incremental baseline.

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
- building the exact merged commit under a provisional candidate ref
- preparing and signing the publisher's exact artifact set
- tagging that commit only after its workflow and publisher preparation succeed
- waiting for the target's configured completion evidence

The repository owns `verify` and `prepare` commands, the selected candidate
workflow, and the publisher under `release.targets` in `.lf/config.yaml`. Keep
credential-free builds and smoke tests in the workflow. Keep signing,
publication, deployment, and secret handling in the publisher.

## Re-entry

`lf release run` resumes an existing release PR or incomplete latest tag after
interruptions.

A minor run records its selected patch, minor version, and source snapshot in
`.lf/releases/minor-<target>.json`. Retry `lf release run minor` to complete that
pair; a published patch is reused even when no unreleased commits remain.

Process death does not make generated release state temporary. Re-entry owns:

- provisional `release-candidate/<target>/<tag>/<commit>` refs
- generated `prepare-<target>-<tag>-<commit>` and
  `publish-<target>-<tag>` branch/worktree pairs
- exact prepared artifacts at
  `.lf/releases/<tag>/<commit>-<workflow-run-id>`

Before creating, reusing, or removing one of these intermediates, the command
must observe its target, tag, source commit, workflow run, generated path/ref,
Git cleanliness and registration, and current stage lease. It then follows one
restart rule: **observe → classify → converge or refuse**. Under the exact stage
lease, reuse or replace only matching inactive state. Mismatched, dirty,
differently registered, or live-owned state fails closed with the observed
ownership evidence and no mutation.

The one-shot caller must not manually remove a stale generated worktree, ref, or
artifact directory as a workaround. A release stage is resumable only when each
durable intermediate implements this restart rule.

## Guardrails

- The version comes from the user or wave config. Don't decide it.
- Be concrete in release notes: include real shipped changes, not vague summaries.
- If required data is missing (tags/gh auth/workflows), state exactly what command failed and what to run.

## Adaptation

If you discovered repo-specific release conventions — changelog format, tag
scheme, deploy hooks, version file locations — encode them in `.lf/config.yaml`
or repo-owned scripts. Copy this skill to `.lf/skills/release-run.md` only when
the release judgment itself needs to differ.
