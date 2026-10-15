# v0.13.16

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.16 removes setup and cleanup steps that interrupt ongoing work. Publish follow-up changes from the same Task checkout after a merge, create a repository's first release without a seed tag, and keep new worktree names consistent after renaming the primary checkout.

## Keep delivering from the same Task

A merged PR no longer requires another Task or manual branch cleanup before publishing the next change. Publication preserves earlier PR history and carries follow-up work past squash merges, including when the remote branch was pruned.

- Run `lf pr publish` from the existing checkout to create the next PR. Repeated publication updates that PR rather than creating another.
- Local edits and already-pushed follow-up commits survive branch continuation. Remote history is replaced with a lease only when the exact delivered head is still present; divergent or concurrent remote work remains intact.
- Creating the successor PR reopens a completed Task and clears its earlier completion intent. An earlier landing cannot complete the new delivery.
- `lf pr submit` creates a missing PR without `--create-pr`. Repeating `lf land` prepares another PR when new work exists and does nothing for an unchanged merged delivery.

## Make the first release directly

An explicit initial version now uses repository history when no preceding release exists. There is no need to manufacture a seed tag or roll back an existing manifest version to start the release lifecycle.

- Use `lf release notes 0.1.0 --preview` in a configured repository without release tags for the selected target. The baseline is repository start.
- Initial notes and previews include the root commit and follow first-parent history with target-area filtering. An explicit `--prev-tag` still takes precedence.
- First releases use the existing single-candidate lifecycle, preserving candidate and tag identity across retries. Relative versions still require a baseline; subsequent minor-cycle behavior is unchanged.

## Keep checkout names predictable after a rename

New sibling worktrees now take their prefix from the primary checkout's current directory name. Older siblings with different prefixes no longer cause Loopflow to drop the repository name from new paths.

- Renaming `etude` to `fantasia` makes future siblings use names such as `fantasia.managym-cpu-research`, including when creation starts from an older linked checkout.
- Existing worktrees stay in place. Dedicated host-directory inference and reuse of an existing branch checkout remain supported, without rename-history configuration.

## Operational notes

- Publication can reopen a completed Task; launching completed Tasks is unchanged. Integrating unrelated upstream changes still requires sync.
- Delivery continuation and first-release recovery were checked with real Git repositories and simulated hosting. Live GitHub publication and installed acceptance remain unverified in the supplied record. The worktree rename regression exercised real CLI creation across successive renames while retaining dirty work.
- Focused tests, formatting and Clippy passed for the included changes. The supplied record does not establish a completed hosted full-matrix pass for the release.

## Small changes

- Task checkout names now use the same normalization as `lf wt create`.
- Task checkout creation or restoration precedes explicit agent selection. Reconciliation skips deleted directories while retaining the Task, PR and landing intent.