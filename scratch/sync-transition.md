# Sync transition evidence — 2026-10-02

Jack authorized pursue through demo with local scratch preservation. Installed
sync successfully merged main (562764212, aabec7759; 18 focused tests passed),
then stash restoration collided with scratch/questions.md written by its conflict
resolver. Tracked compression edits were restored; untracked scratch was retained
in stash 84889bddd22f66f5d2d38427f8d5a3a945adb4e0. The supervising Session read that
stash and restored both complete design files, retaining the resolver finding.
No source or notes were discarded. Earlier apparent scratch absence was the
in-progress stash, not proven data loss.

Realignment repaired the scheduled-install command path. Inspection confirmed
that the CLI sync wrapper still uses `with_preserved_edits`, whose stash restore
can collide with resolver-created files. Remaining recovery work and publication
constraints live in wave-and-repo-worktrees.md.

2026-10-02 repair: checkout restoration now moves colliding resolver notes aside
before applying the stash. Both note versions remain local. The saved Flow decoder
already adapts removed command owners; populated-store coverage now verifies it.

Local code review findings: reserve sidecar names present in the stash so a retry
cannot overwrite prior resolver notes; handle blocking parent files and symlinks
before descending; keep tracked collisions in Git recovery. Fixed stale literal
command examples in troubleshooting, Task operating guidance, builtin graph tests,
the simulated landing provider, PR Flow-command fixtures, and the Session activity
lookup. Required checks exposed these removed spellings; the retained behaviors
are tested through their surviving literal paths. No new recovery owner or migration was needed.

Exact headless repair demos (temporary fixture repositories and stores):

```bash
cargo test -p loopflow --test sync_tests checkout_restoration_preserves_resolver_notes_index_and_retry -- --nocapture
cargo test -p loopflow --test sync_tests resolved_sync_restores_original_scratch_through_checkout_boundary -- --nocapture
cargo test -p loopflow --lib saved_flow_commands_migrate_without_changing_the_cursor_or_identity
```

Deferred configured-runtime demo, after the provider schema fix is available and
Jack authorizes the external actions: `lf session ensure`,
`lf session ensure --wave <named-wave>`, `lf session replace <session-id>`, then
`lf sync --plan` and `lf sync` in each resident checkout. Inspect authored documents
with `git diff -- <document-path>`, checkpoint with
`lf commit -m "Record accepted decisions" <document-path>`, inspect
`git diff origin/main...HEAD --stat`, and publish with `lf pr publish`. After an
independently authorized merge, run `lf sync` and verify local scratch and later
edits remain. No Wave is inferred and no merge or review completion is authorized
by the bounded repair. These deferred commands were not executed.
