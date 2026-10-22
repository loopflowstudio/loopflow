# v0.13.7

<!-- loopflow:release-notes=narrative;gate=safe -->

v0.13.7 keeps work moving through everyday interruptions: opening a Wave, returning to a Task, installing an update, and carrying follow-up work into its next PR. Desktop prepares the selected Wave Project on opening and preserves the selected conversation when reopening a Task. Backups progress while the Home remains active, and release preparation can resume a retained notarization submission after a timeout.

## Open a Wave ready for work

Opening a Wave could leave its plan unusable until someone manually activated its Project. Desktop now prepares the selected Project on opening, shows preparation failures with Retry, and shares the saved Project identity with commands and other checkouts.

- Use `lf wave bind-project` to select a Project and `lf wave ensure` to prepare it. Preparation can activate or create a Project; creation retries reuse the reserved identity.
- Existing Linear names and optional Flow settings are preserved. Legacy YAML Project selections are imported once into SQLite.
- Cached planning and independent Session reads remain available during preparation. Passive status reads do not create Projects.
- **Realign Projects…** previews a retained plan and applies the reviewed version. Rotation requires exact destinations and authored KRs, moves started unfinished Tasks with their checkout, PR and execution intact, and leaves unreviewed backlog in its original Project.

## Return to the conversation you left

Task links now open directly and prefer the window already holding their destination. Reopening a Task preserves the selected conversation and pane layout while using current planning rather than stale link results.

- Unknown or ambiguous destinations retain resolution and retry behavior; unknown Sessions load every page.
- Opening a Task no longer probes missing historical checkout paths. Automatic workspace updates remain intact.
- The recorded nine-attempt demo measured workspace construction around 1.7 seconds cold, 160 ms for warm opens, and 56 ms for reopens. These are workspace measurements, not cold application launch, p95, keyboard-focus, or provider-readiness guarantees.

## Keep maintenance from stalling work

Installation and follow-up PR creation both had ways to stop despite usable work being available. Backups now keep progressing during Home writes, and `lf pr next` handles upstream changes already present after syncing main.

- Compatibility-check backups use a pinned read snapshot so ongoing writes do not repeatedly restart the copy.
- Migration backups announce the backup phase and copy larger batches without deliberate pauses.
- Follow-up carries walk first-parent history and apply merges relative to their first parent, dropping changes already present on the new base instead of stopping on an empty cherry-pick.
- Edits recorded in merge commits and intentionally authored empty commits are preserved. A real conflict aborts the complete carry and restores the original branch without losing work.

## Operational notes

Release preparation now retains the exact signed DMG, source commit, SHA-256 and Apple submission ID beside the prepared-artifact directory. A notarization wait timeout can be retried against that same submission after checkout cleanup, without rebuilding and uploading again.

- The submission receipt is saved before the long wait. Stapling uses a copy so the retained submitted bytes remain verifiable; changed artifacts and unknown upload outcomes remain explicit failures.
- This does not recover the earlier live v0.13.7 attempt: its DMG had already been removed under the previous code, and Apple still reported it In Progress in the supplied evidence.
- Update monitoring commands to `lf monitor work`; the former `lf monitor workspace` spelling is removed. Desktop retains saved navigation and layout.
- Project rotation retains recovery records within its owning Home, but provider mutations are not atomic.
- Recorded checks include backup and carry regressions with Clippy, 17 Swift destination tests, 13 Python benchmark tests, and 54 release-script and publisher tests. Installed backup validation and the blocked live Task rotation retry remain outstanding.
- Project preparation and realignment passed recorded formatting, architecture, migration and Swift platform-boundary checks. Affected suites, Clippy and app builds were deferred for insufficient disk reserve. Public CLI/crash acceptance remains unimplemented; configured-provider and mounted Desktop acceptance remain unproved.

## Small changes

- Release CI fixtures now tolerate valid observation timing changes while retaining Task-deletion preservation and exact account-selection checks. This changes test expectations, not production behavior; the account fixture does not prove native resume.
- Worktree-listing benchmark documentation retains installed evidence and the still-unmet one-second latency target.
- GhosttyKit documentation records Jack Heart's standing authorization for verified artifact publication.