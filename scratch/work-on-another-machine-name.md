# Share Task planning across machines — LOO-412

## Accepted direction — Jack Heart, 2026-10-08

Jack Heart selected local planning stores with bidirectional synchronization
through one custom Git ref, publication of PR #1491, and review in the existing
conversation. Steers `f883b34b-1195-4fc5-bfc7-1ac7d4c29be6` and
`1d12770f-96b1-4bc3-ba5b-1f863f4b9472` supersede host callbacks. The earlier
design is retained at `b040c7c8d:scratch/work-on-another-machine-name.md`; the
adoption demo remains at `0cd8e7f14` and in `remote-task-demo.md`.

LOO-412 owns Git transport and machine integration. LOO-406 owns the common
local SQLite planning writer and optional repository-wide Linear synchronization.
Only coherent committed APIs may be integrated, never its dirty implementation.
Ordinary operations do not call back to another machine or use another planner.
No real planning publication to the public code remote is selected. Prototype
acceptance uses synthetic records and disposable remotes. No landing or installation.

## Experience and preservation

```sh
lf --machine mini task create --title "Fix the parser"
lf --machine mini --task <task-selector> skill implement
```

Each machine reads and writes its own local plan. Synchronization shares stable
Task identity, briefs, comments and planning disposition in both directions.
Creation needs no checkout, Session or Flow. A later execution placement uses
the already pushed branch/commit; repeat launches reuse the worker's checkout.
Comments and completion propagate during ongoing work, with visible pending or
unconfirmed synchronization when disconnected.

Execution stays local. Never replicate Workflow position, Sessions, Processes,
claims, controls, checkout paths or PR execution state. Imported completion cannot
move a local Workflow, settle execution, signal a process or claim an exit.
Machine/account operations execute on their selected machine. No shared resident,
terminal relay, automatic turn/Flow retry, hidden arguments or cross-version
compatibility is included. Jack excluded code, tests, help and config from herdr/cmux.

Ordinary creation establishes identity once; replication preserves it. Retrying
one creation retains identity while separate same-title creations remain distinct.
Existing divergent IDs and their execution histories are never renumbered. Use
provider mappings or the smallest explicit association where necessary; matching
titles do not establish identity.

Preserve fetch-before-decision and pushed-code checks. Report uncommitted source
work, missing branches and unpushed commits by branch/commit without committing,
pushing or resetting. Dirty/behind target checkouts retain files, HEAD, IDs,
Workflow, Sessions and PR history. A behind target reports `lf sync` in its
checkout. Creation without code has no fabricated commit or placement. The
one-shot code requirement constrains initial placement, not later local commits.

## Reconciliation and transport

Jack Heart's October 8 conflict decisions in LOO-406 supersede mandatory manual
winner selection: Linear wins observed planning conflicts. Without Linear, prefer
the host where appropriate, otherwise last-write-wins with try-not-to-clobber and
recoverable losing edits. Independent creations/comments accumulate and different
fields survive. A delayed completion must not overwrite an observed newer reopening.
The existing causal exchange module is an implementation starting point, not an
accepted requirement to expose manual conflicts or to block on LOO-406 APIs.
Simplify or replace it to implement the selected policy; retain original evidence.

Extend the common SQLite writers here with the peer ordering, stable identities and
atomic import/export checkpoint needed by Git synchronization. Export must not mint
new mutation identities. Import and projection commit together; fetched Git history
is not an import acknowledgement. Crashes before commit leave import retryable.
Pending state distinguishes local save, Git publication and optional Linear delivery.

`engine/planning_git.rs` requires an explicit user-keyed or shared ref under
`refs/loopflow/planning/`. Retained/observed refs are scoped by remote/ref; changing
the selected ref cannot reuse another plan's unpublished history. Publication checks
that the revision belongs to that destination, never forces or blindly retries,
and distinguishes confirmed, competing and unconfirmed readback. Fetch preserves
source branch, index, checkout and `FETCH_HEAD`. The 16 MiB document bound and
30-second Git deadline remain prototype engineering choices.

The common SQLite writer now captures immutable field mutations in the same
transaction using planning-only triggers. Export reads those stable identities.
Causal predecessors retire observed heads; concurrent Linear-origin observations
win, otherwise logical time and change ID decide. All losing values remain in the
journal, not only in Git history. Task disposition and comment content are atomic
field groups. A derived heads index is checked against the journal on export.

Peer import unions the journal and projects into the existing Wave/Project/Task
and comment tables with one import checkpoint. Projection suppresses capture echo,
advances Task optimistic revisions only on changed planning, and never calls an
execution writer. Repository paths are supplied locally, not transported. The
single `planning_peers.sql` draft depends on the unchanged parent `local_planning`
draft; it seeds existing IDs once. These are source changes, not passing Rust proof.

## Delete — do not maintain

The copied `TaskSource.planning` payload, issue-derived `TaskId::from_issue`, UUID-v5
feature and exclusive adoption tests are removed. SSH now carries saved Task ID,
identifier and pushed-code requirements only. Placement consumes already imported
planning and retains branch/commit and dirty-checkout checks. The remote fixture
uses explicit peer import before dispatch; it does **not** prove automatic sync.
The earlier walkthrough/captures remain archived at `5960415b3`.

The manual-conflict-only `PlanningField` model is replaced by immutable mutations
and deterministic projection. No full-store callback or second planning store
exists. Docs describe the unfinished public path rather than claiming adoption.

## Remaining integration — October 8

1. Bind an explicitly selected planning remote/ref locally, provision/recover one
   stable user key across machines, and implement safe shared joining. A code remote
   is never an implicit destination. Existing local plans must not be silently
   exported or merged when joining.
2. Compose Git fetch, transactional import/export, retained publication and readback
   with foreground invocation lifetimes. Add selected-plan and pending/unconfirmed
   presentation. The current library components do not exchange on ordinary commands.
3. Resolve divergent legacy IDs through explicit provider associations without
   renumbering stored Work. Review found that the current importer rejects a duplicate
   provider mapping or a protected ancestry move for the whole transaction. Retain
   failed projection/conflict evidence while letting unrelated objects progress;
   do not weaken the preservation constraints or treat this as completed acceptance.
4. Complete optional Linear composition: peer imports must preserve/settle the common
   pending receipts correctly, including unchanged observed baselines and late local
   saves. Verify every provider-projection entry point supplies Linear provenance;
   migration baselines are not fresh provider observations.
5. Finish public cold-worker dispatch after automatic acquisition, local-born IDs,
   legacy aliases and populated execution preservation. The earlier callback path
   is gone; unknown remote planning currently produces a synchronization diagnostic.
6. Run the focused Rust checks, then gate's complete acceptance and the final
   walkthrough before republishing #1491. No publication, landing or installation
   has occurred in this implementation pass.

## Integration now selected — 2026-10-08

Jack Heart requested stacking on LOO-406 and continuing pursue. `lf task checkout
LOO-412 --stack-on LOO-406` selected PR #1503; `lf sync --manual` integrated its
published `e68f2a423bdabf91acadea72f23f01b3f383f244`. The common ownership cut
`84664e661` deletes `PlanningAuthority`, personal-plan storage and split writers.
Saved readers, ordinary creation/edit/comment/disposition and placement now share
local identity. This supersedes the missing-writer diagnosis at `4f9a8ea17`.

LOO-412 owns peer export/import, ordering and checkpoints on those writers. These
are implementation work here, not a new dependency for LOO-406 to deliver. Do not
wait for all Linear delivery to finish or create a second planner. Source evidence:
`e68f2a423:scratch/explore-loopflow-s-own-store.md`, common ownership, preserved
writers, accepted conflict policy and user-keyed planning sections.

The merge uses saved Task placement, retaining fetch-before-placement and source
branch/commit checks. Tasks may exist without checkout. The obsolete copied-record
importer and cold Project selector are removed; new peer sync must provide missing
planning before launch. Automatic peer acquisition and the remaining legacy-association handling
are still absent. This intermediate integration is not ready for
publication as a working remote path; complete the end-to-end cut before review.

Jack's accepted destination policy: user-keyed planning by default, explicit opt-in
to a common shared plan, same records/APIs. A common code remote never combines
plans implicitly. LOO-412 owns stable key provisioning/recovery and local remote/ref
binding; never derive identity from Git display name or a per-machine random user.
Joining an existing plan must not silently upload or merge the current local plan.
Shared ordering cannot grant an initiating host blanket precedence over other
collaborators. Choose deterministic ties and retain losers. Ref separation is not
access control; private planning needs an access-controlled destination. Synthetic
fixtures remain the only authorized publication target.

The remaining-integration list above owns the unfinished cut. LOO-406's known
unseen-Linear-write race remains contrary evidence about provider atomicity,
not a missing product decision or reason to block independent peer integration.

## Acceptance for review

Use CLIs built from this worktree, isolated stores, synthetic plans and disposable
remotes. Installed-release behavior remains separate. Release's child memory
records a recovery path missed by lower-level fixtures; the same lesson requires
public command coverage here, not only exchange and transport tests.

1. Create/edit/comment/read from either machine through public commands. Replicate
   the same ID without placement from creation. Repeated deliveries create no
   duplicate Task/comment; separate same-title creations remain distinct.
2. Show semi-live comments/completion during remote work. Preserve populated
   Workflow, Session, Process and PR history/controls on imported completion;
   unrelated plans and progress remain usable.
3. Disconnect, write locally, show pending state, reconnect and converge without
   loss or turn replay. Lose a publication response and recover by readback. Crash
   after fetch but before import, then prove atomic retry.
4. Preserve conflicting edits/dispositions while independent updates propagate.
   Apply the selected deterministic policy and retain losing edits; delayed
   completion cannot undo observed reopening. Cover
   equal-value concurrent writes, conflicting ID reuse, omission and deletion.
5. Public `--machine` launches by issue/local selector and label/ID on a blank
   worker see pushed code and reuse placement. Cover local-born identity,
   divergent legacy IDs, unpushed source, missing branch and behind/dirty target.
6. Exercise local-only planning and contained optional Linear pending sync through
   the common writer. A local bare remote does not prove hosted custom-ref policy;
   that requires an explicitly selected disposable hosting repository.
7. Replace PR copy and create a walkthrough of final behavior and evidence;
   publish #1491 for Jack Heart's review and stop without landing.

Check (October 8): `cargo fmt --all -- --check`, `git diff --check`, and isolated Python SQLite replay (released SQL + parent draft + populated peer upgrade/capture/rollback) pass; Rust build/focused binaries and `cargo clippy --all-targets -- -D warnings` remain unverified because the build-script executable stalled at `_dyld_start`, including fresh executable/path probes; the bounded Clippy attempt expired. Gate needs a capable runner. Earlier passing evidence remains at `5d336868f:scratch/work-on-another-machine-name.md` and does not cover this cut.
