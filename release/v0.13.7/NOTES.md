# v0.13.7

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.7 keeps work moving when opening a Wave, installing an update, or carrying a Task's follow-up into its next PR. Desktop prepares the selected Wave Project on opening and offers explicit realignment that preserves ongoing work. Installation backups progress while the Home remains active, and follow-up carries handle upstream changes already present after syncing main.

## Open a Wave ready for work

Opening a Wave previously could leave its plan unusable until someone manually activated its Project. Desktop now prepares the selected Project on opening, shows preparation failures with Retry, and shares the same saved Project identity with commands and other checkouts.

- Use `lf wave bind-project` to select a Project and `lf wave ensure` to prepare it. Preparation can activate or create a Project; creation retries reuse the reserved identity.
- Existing Linear names and optional Flow settings are preserved. Legacy YAML Project selections are imported once into SQLite.
- Cached planning and independent Session reads remain available during preparation. Passive status reads do not create Projects.
- **Realign Projects…** previews a retained plan and applies the reviewed version. Rotation requires exact destinations and authored KRs, moves started unfinished Tasks with their checkout, PR and execution intact, and leaves unreviewed backlog in its original Project.

## Keep installation backups moving

Installing an update could appear hung when ongoing writes repeatedly restarted the Home copy used for candidate compatibility checks. Preflight copies now use a pinned read snapshot, while migration backups copy larger batches without deliberate pauses.

- Compatibility-check backups progress while writes continue.
- Migration backups announce the backup phase and remove sleeps between batches.
- A regression test copies a 64 MiB database during continued writes and verifies the copy's contents and integrity.

## Carry follow-ups after syncing main

A landed Task could sync main, gain a follow-up commit, then fail during `lf pr next` because the carry replayed upstream commits already on the new base. The carry now follows the branch's first-parent history and applies merges relative to their first parent.

- Changes already present on the new base are dropped instead of stopping on an empty cherry-pick.
- Edits recorded in merge commits and intentionally authored empty commits are preserved.
- A real conflict aborts the complete sequence and restores the original branch without losing work.

## Operational notes

- Update monitoring commands to `lf monitor work`; the former `lf monitor workspace` spelling is removed. Desktop retains saved navigation and layout.
- Project rotation retains recovery records within its owning Home, but provider mutations are not atomic.
- Recorded backup and carry checks include focused regressions and Clippy. Installed validation remains outstanding: the backup repair has not been checked through an installed release, and the blocked live Task rotation has not been retried with the repaired CLI.
- Project preparation and realignment passed recorded formatting, architecture, migration and Swift platform-boundary checks. Affected suites, Clippy and app builds were deferred for insufficient disk reserve. Public CLI/crash acceptance remains unimplemented; configured-provider and mounted Desktop acceptance remain unproved.

## Small changes

- Worktree-listing benchmark documentation retains installed evidence and the still-unmet one-second latency target.
- GhosttyKit documentation records Jack Heart's standing authorization for verified artifact publication.