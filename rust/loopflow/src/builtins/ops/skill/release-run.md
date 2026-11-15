---
requires: optional version input in message text (patch|minor|major|vX.Y.Z)
produces: RELEASE_NOTES.md, tagged release
diff_files: false
---
Cut a release end to end with one command.

## Input

`lf skill release-run <version>` passes `<version>` as message text. The first
token is `patch`, `minor` or `major` (bump from the latest tag) or an exact
`vX.Y.Z` / `X.Y.Z`. No input means `patch`.

`minor` cuts from current main like any release, including changes merged
since the latest patch. Its notes cover the cycle since the preceding `.0`
tag; patch notes cover only what's new since the last patch.

## Run

```bash
lf repo release run <version>
```

That command:
- reads the target's git range since the previous tag
- bumps manifests and generates release notes
- promotes staged `release/unreleased/` files to `release/v<version>/`
- archives the root `RELEASE_NOTES.md` to `release/v<version>/NOTES.md`
- creates and lands the release PR, and waits for the merge queue
- builds the exact merged commit under a provisional candidate ref
- prepares and signs the publisher's exact files
- tags that commit only after its workflow and publisher preparation succeed
- waits for the target's configured completion check
- for a `github-release` target, moves GitHub's Latest marker (the `stable`
  pointer) to the newest release published at least 7 days ago that no GitHub
  issue labelled `regressed-in:v<version>` covers, even when there was nothing
  to release; when those issues can't be read, `stable` stays put

The repository's `.lf/config.yaml` defines, under `release.targets`, the
`verify` and `prepare` commands, the candidate workflow and the publisher.
Credential-free builds and smoke tests go in the workflow; signing,
publication, deployment and secrets go in the publisher.

## Regressions and stable

```bash
lf repo release regress v0.15.3 "Sessions lose their name"  # file one against a release
lf repo release issues v0.15.3                              # what it carries, and what each issue holds
lf repo release promote --dry-run                           # what would move, or what holds it
lf repo release promote                                     # move stable now, outside a run
```

A GitHub issue labelled `regressed-in:v<version>` is all a regression takes.

## Re-entry

`lf release run` resumes an existing release PR or an incomplete latest tag. An
unpublished preparation commit doesn't use up a version: fix it with another
commit under the same version. Never bump the version to escape a failed
build, a late migration, or an interrupted publication.

Process death does not make generated release state temporary. Re-entry picks up:

- provisional `release-candidate/<target>/<tag>/<commit>` refs
- generated `prepare-<target>-<tag>-<commit>` and
  `publish-<target>-<tag>` branch/worktree pairs
- exact prepared files at
  `.lf/releases/<tag>/<commit>-<workflow-run-id>`

Before creating, reusing or removing one of these, the command reads its
target, tag, source commit, workflow run, path or ref, Git cleanliness and
registration, and the current stage lease, then follows one rule: **observe →
classify → converge or refuse**. Under the exact stage lease it reuses or
replaces only matching inactive state. Mismatched, dirty, differently
registered, or live-owned state fails closed: it reports what it found and
changes nothing.

You must not manually remove a stale generated worktree, ref, or artifact
directory as a workaround. A release stage can resume only when each of its
intermediates follows this rule.

## Guardrails

- The version comes from the request or Wave config. Don't pick it.
- Release notes name real shipped changes, not vague summaries.
- When data is missing (tags, gh auth, workflows), say exactly which command
  failed and what to run.

## Adaptation

Put repository release conventions (changelog format, tag scheme, deploy
hooks, version file locations) in `.lf/config.yaml` or repository scripts.
Copy this skill to `.lf/skills/release-run.md` only when the release judgment
itself needs to differ.
