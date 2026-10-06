# v0.13.8

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.8 repairs follow-up PR creation when main advances again after a settled Task has synced it. The carry now preserves follow-up edits and merge resolutions without replaying upstream changes that are already integrated.

## Carry follow-up work after main moves again

The previous carry could replay old upstream changes and conflict instead of moving the Task's follow-up work onto the new base. This patch corrects how already-integrated merges are replayed during rotation.

- `lf pr next` replays an already-integrated merge relative to its incoming parent, preserving branch edits and merge resolutions.
- If any later commit conflicts, the entire carry is rolled back to restore the original branch.

## Operational notes

This addresses the second rotation failure observed for LOO-375 on installed v0.13.7. Two regression tests passed, covering later upstream edits with and without merge-authored changes, and full rollback after a later conflict. Formatting and `cargo clippy --all-targets -- -D warnings` also passed.

Installed rotation and Task performance acceptance still require verification after delivery.