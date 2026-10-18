# LOO-497 — continue on the LOO-489 Console

Jack Heart requested integrating/rebasing onto LOO-489, rebuilding this Task’s
prototype from its UX, and then said “keep going” after correcting the prototype
query. Continue implementation through the current pursue edge. Do not stop again
merely because 489 has not committed its working tree: the bounded source snapshot
below supplies the missing handoff. No product permission is missing for routine
integration. No merge/auto-merge authorization follows.

## Materialized handoff

`scratch/history/loo489-console-source-2026-10-17/MANIFEST.json` lists 108 exact
changed source files from LOO-489 at HEAD
`ac973064c691581610fe563364ddf1a4f239798c`, with a second read verifying every hash.
`source/<path>` contains the final source; `base/<path>` contains the corresponding
HEAD version if one existed. `source_snapshot`/`base_snapshot` give exact paths;
Markdown source is stored as `.md.txt` to avoid automatic context injection.
Deleted files are explicit. The source checkout and
its index were untouched. The complete current navigation contract, native demo,
keyboard map and earlier tab handoff accompany the snapshot as `.txt` files.
The local `scratch/navigation-console-contract.md` is also current.

Fresh reads found 489's latest pursue `72927d43-5e96-48ac-b1a9-134b4836a708`
completed at demo, and no live 489 Flow. Its source is ahead of its branch HEAD.
The snapshot is a supported integration input, not passing combined evidence.
Don't mutate the sibling checkout or launch another 489 Flow.

## Integration order

1. Preserve all existing 497 contributions. Checkpoint the current shared
   Dashboard/API/tab work through `lf commit` after its required checks. Use the
   installed `lf sync <489-branch> --manual` path for committed ancestry when
   useful; its plan is merge_target on this protected branch, not a history rewrite.
2. Reconcile the captured native changes into this checkout through their existing
   owners. Use base/source/current three-way comparison; don't replace destination
   files wholesale. 489 already adopted a bounded snapshot of our tab model, so
   preserve our newer Dashboard/persistence contracts while taking its Console
   navigation, renderer extraction, current Session preview and provider repairs.
   Source `work/selection.rs` and `WorkSelection.swift` are predecessor consumers,
   not authority to delete our separate Dashboard model. Remove displaced native
   implementations instead of retaining both. Keep 489's actual rounded pane cards,
   borders/shadows/gutters accepted in its latest native demo notes.
3. Wire separate native Dashboards into that navigation and the existing retained
   Task workspaces. Use Rust plans, not a Swift evaluator. Normal repository opens
   its conversation; normal Wave opens conversation + full current Task inventory.
   No custom selection replaces either. Stable working Task order is independent
   of Dashboard query ordering; paused Tasks stay queryable while leaving working
   slots. Cmd K is Find Work, including Dashboards and cross-repository windows.
4. Complete native query editing and portable save/restore against the existing
   shared Dashboard/Work intent APIs. 497 owns this Dashboard adoption in the
   combined source; don't wait for 489 to build our API/editor. Preserve newer
   tabs/drafts on delayed reads, expose save conflicts, and retain one native owner.
   Keep 416's cross-launch attachment/lifetime boundary. Capture only a real,
   specific unresolved transport dependency if encountered; do not infer a new
   shell launch from saved intent or introduce another lifetime registry.
5. Run focused combined build/tests, then the authored compression/refresh/publish
   steps. Keep task disposition honest: no native demo claim from HTML or model
   tests. Publish for review only; no merge/arming.

## Latest query correction

Jack Heart rejected `query:product or query:available`: those were invisible
hardcoded fixtures. The revised prototype uses
`(wave:product state:open) or state:available`. Membership is explicit; no `repo:`
predicate. This does not prohibit genuinely authored Dashboard references through
the shared API, but the UX must identify their owner; do not invent aliases.

Prototype code and screenshots are in `scratch/query-view`. Preserve it and the
Task’s existing draft/history. It is a design study, not production rendering.
Earlier “missing combined handoff” notes are superseded by this snapshot.
