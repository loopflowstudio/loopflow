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
Machine selection also routes account and machine operations. Jack excluded shared
residents, terminal relays, automatic turn/Flow retries, hidden arguments, cross-version
compatibility, and code, tests, help or config lifted from herdr/cmux.

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
The implemented causal exchange selects winners and retains losing mutations;
it is not a manual-conflict-only protocol. Projection conflicts describe records
that cannot enter the local tables, independently of field winner selection.

The common SQLite writers now retain peer ordering, stable mutation identities and
an atomic import checkpoint. Foreground Git synchronization remains unimplemented.
Export reads existing identities rather than minting edits. Import and projection
commit together; fetched Git history is not an import acknowledgement. Crashes before commit leave import retryable.
Pending state distinguishes local save, Git publication and optional Linear delivery.

`engine/planning_git.rs` resolves an explicitly chosen remote alias once into a
pinned endpoint and user-keyed/shared ref under `refs/loopflow/planning/`. Separate
fetch/push endpoints cannot supply a valid readback. Retained/observed refs are
scoped by the saved endpoint/ref; changing an alias cannot redirect a saved binding
or reuse another destination's unpublished history. Publication checks
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
and comment tables with one import checkpoint. Expected uniqueness, missing-parent
and protected-ancestry failures now roll back only that object's projection. The
journal and local conflict receipts retain unprojected identities, including their
comments, for export and later projection. A repaired association can settle on
another acquisition without new mutations. No partial placeholder Task remains.
Wave selection follows Project projection; unresolved selection preserves the old
selection. A bounded parent pass handles ordering, not turn or Flow retries.

The checkpoint acknowledges retained mutations and explicit projection conflicts,
not that every record projected. Active conflicts have a Store reader and publish
planning revisions only when their state changes; historical reasons survive
resolution. Invalid documents, reused IDs, cross-repository ownership and unexpected
SQL failures still roll back the whole import. The common writer suppresses echo,
advances Task optimistic revisions only on changed planning and calls no execution
writer. Paths stay local. The single `planning_peers.sql` draft still depends on
`local_planning`. Destination binding and membership now live in the same draft. Joining is empty;
export joins the mutation journal to explicit selected-record membership, never to
all repository rows. Selecting a Wave includes its descendants; new descendants
inherit that membership. Imports reserve incoming identities transactionally and
refuse overlap with unselected live records or retained journals. References cannot
attach selected work to an unselected parent. Conflict settlement is destination
scoped. An explicit canonical user UUID can be provisioned once or recovered on
another store; connecting never invents a per-machine user. These are source
changes, not passing Rust proof.

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

Earlier reductions removed the duplicate SSH parse, Task preparation path and
mutation decoder; exact rationale and ref-rejection repair are retained at
`81fcf66e3:scratch/work-on-another-machine-name.md`. The importer now checks only
new receipts against global mutation identities; the snapshot merge already checks
retained local receipts. Explicit inserts replace silent conflict ignores. Projection
borrows fields instead of cloning each object's values, and Wave validation is
explicit rather than inferred from a field's presence. Per-object savepoints and
immediate foreign keys retain conflict isolation. Conflict revision triggers now
use the store's table-based naming contract and cover deletion as well as saves.
The missing-code fixture no longer installs unrelated Linear/Wave configuration.
Repository-wide `export_peer_planning(repo)` and alias-addressed transport
construction are removed; every export requires a saved destination and its selected
records. No parallel store or implicit repository-wide publication remains.
Automatic exchange remains unfinished.

## Remaining integration — October 8

1. Expose destination/key setup and active-plan selection through public commands.
   Store APIs now pin endpoint/ref, provision/recover an explicit stable user key,
   bind empty selections and select whole Waves. New descendants inherit selection;
   root-Wave creation still needs explicit active-plan routing. No setup UX generates
   or transfers a key yet. Shared joining must preserve existing unselected work,
   including overlapping IDs and retained journals, without uploading it. Disposable
   remote regressions are authored but Rust execution is unproved. No real plan
   publication is authorized.
2. Compose Git fetch, transactional import/export, retained publication and readback
   with foreground invocation lifetimes. Add selected-plan and pending/unconfirmed
   presentation, authorship and assignees. No production command currently calls
   `PlanningGit` or peer import/export. Acquisition must precede cold Task resolution;
   a foreground observer must continue during a long-running command, without a
   resident or automatic turn retry. A successful fetch, retained import checkpoint
   and confirmed publication are separate outcomes.
3. Resolve divergent legacy IDs through explicit provider associations without
   renumbering stored Work. Conflict isolation is implemented with new regressions
   for duplicate mappings, dependent comments, retained Sessions, selection and
   idempotent retry, but Rust execution remains unproved. Wire the conflict reader
   into foreground status; a durable import checkpoint is not completed projection.
4. Complete optional Linear composition. `project_fields` updates planning rows
   directly; it neither creates nor reconciles `task_changes`, `project_changes` or
   comment/disposition delivery receipts. Retaining those tables unchanged is not
   enough: an older pending write must not later replace a peer winner, and imported
   local edits need delivery through the common writer without echoing Linear.
   Preserve attempted identities, uncertainty, unchanged baselines and late saves.
   Only `project_accepted_planning` currently sets the Linear-origin context; cover
   other provider projection paths and equal-value observations, which the peer
   update triggers omit. Migration baselines are not fresh provider observations.
5. Finish public cold-worker dispatch after automatic acquisition, local-born IDs,
   legacy aliases and populated execution preservation. The earlier callback path
   is gone; unknown remote planning currently produces a synchronization diagnostic.
6. Run the focused Rust checks, then gate's complete acceptance and the final
   walkthrough before republishing #1491. No publication, landing or installation
   has occurred in this implementation pass.

## Committed integration boundary — 2026-10-08

Jack Heart requested stacking on LOO-406 and continuing pursue. Parent PR #1503's
published `e68f2a423bdabf91acadea72f23f01b3f383f244` is integrated; its common
ownership cut `84664e661` removed `PlanningAuthority`, personal-plan storage and
split writers. Source: `e68f2a423:scratch/explore-loopflow-s-own-store.md`.
The missing-writer diagnosis at `4f9a8ea17` is superseded. LOO-412 owns remaining
peer integration independently of unfinished Linear delivery; no second planner.

Jack's destination policy uses a stable user key across machines, never Git display
names or per-machine random users. Ref separation is not access control: private
planning requires a controlled remote. Joining must preserve existing plans without
silently merging or uploading them. Synthetic fixtures remain the only authorized
publication target. LOO-406's unseen-Linear-write race is contrary provider-atomicity
evidence, not a missing peer-sync decision. Public acquisition, legacy association
and the remaining integration above are required before review-only publication.

## Acceptance for review

Use CLIs built from this worktree, isolated stores, synthetic plans and disposable
remotes. Installed-release behavior remains separate. Release's child memory
records a recovery path missed by lower-level fixtures and a successful failure
report mistaken for operation success. The same lessons require public command
coverage here, and status that distinguishes retained conflicts from convergence.
Its GOAL.md and full MEMORY.md were reviewed on October 8; no other immediate
Infrastructure child memory exists in this checkout.

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
   Joining with unrelated existing plans must neither export nor merge them;
   changing a remote alias must not publish to a newly pointed destination.
5. Public `--machine` launches by issue/local selector and label/ID on a blank
   worker see pushed code and reuse placement. Cover local-born identity,
   divergent legacy IDs, unpushed source, missing branch and behind/dirty target.
6. Exercise local-only planning and contained optional Linear pending sync through
   the common writer. A local bare remote does not prove hosted custom-ref policy;
   that requires an explicitly selected disposable hosting repository.
7. Replace PR copy and create a walkthrough of final behavior and evidence;
   publish #1491 for Jack Heart's review and stop without landing.

Check (October 8 implement): `cargo fmt --all` and `git diff --check` pass; `uv run python /tmp/loo412-selection-sql.py` passes canonical/parent/peer SQL replay, empty join, selected descendant/export isolation and import membership after repairing missing fixture identifiers. Focused `cargo test -p loopflow --lib store::sqlite::planning_peers::tests -- --test-threads=1` stopped after 191 s at build-script `_dyld_start`; `cargo clippy --all-targets -- -D warnings` timed out at 180 s at the same build-script entry. Rust importer/transport/remote fixtures, revision coverage and public acceptance remain for capable gate/CI; SQL is not Rust execution. Earlier evidence: `7af31f09f:scratch/work-on-another-machine-name.md`.

Check (October 8 sync onto `93a587539`): `cargo test -p loopflow --lib store::sqlite::planning_peers::tests::peer_import_keeps_identity_execution_and_concurrent_local_saves -- --exact --test-threads=1` timed out after 180 s during compilation, before test execution; focused Rust proof deferred to capable gate/CI. Merge `f94d3c443` retains both peer and Linear-status modules and the target's cleanup-history note; no push.
