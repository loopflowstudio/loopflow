# v0.13.7

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.7 removes two stalls in routine maintenance: backing up a large, active Home during installation and carrying Task follow-ups after syncing main. Installation backups can keep progressing while writes continue, and `lf pr next` no longer stops on an empty cherry-pick caused by upstream changes already present on the new base.

## Keep installation backups moving

Installing an update could appear hung when ongoing writes repeatedly restarted the Home copy used for candidate compatibility checks. Preflight copies now use a pinned read snapshot, while migration backups copy larger batches without deliberate pauses.

- Compatibility-check backups progress while the Home remains active.
- Migration backups announce the backup phase and remove sleeps between batches.
- A regression test copies a 64 MiB database during continued writes and verifies the copy’s contents and integrity.

## Carry follow-ups after syncing main

A landed Task could sync main, gain a follow-up commit, then fail during `lf pr next` because the carry replayed upstream commits already on the new base. The carry now follows the branch’s first-parent history and applies merges relative to their first parent, preserving follow-up work across sync merges.

- Changes already present on the new base are dropped instead of blocking the carry.
- Edits recorded in merge commits and intentionally authored empty commits are preserved.
- A real conflict still aborts the full sequence and restores the original branch without losing work.

## Operational notes

Recorded checks include focused backup snapshot, integrity and migration-exclusion tests, follow-up carry regressions, and Clippy. Carry tests cover sync merges with and without merge edits, unchanged original history, intentional empty commits, and full rollback after a later conflict.

Installed validation remains outstanding: the backup repair has not yet been checked through an installed release, and the blocked live Task rotation has not been retried with the repaired CLI.

## Small changes

- Updated worktree-listing benchmark documentation and release memory to retain installed evidence and the still-unmet one-second latency target.
- Documented Jack Heart’s standing authorization for verified GhosttyKit artifact publication.