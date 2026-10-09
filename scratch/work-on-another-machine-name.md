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
The original `lf ssh` example is superseded by global `--machine`; identity is
minted once by the common writer and replicated, not derived from an issue name.
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

Each public creation saves and returns a distinct identity, including repeated
same-title requests. Internal delivery and replication retain that saved identity;
there is no public creation token or issue-derived ID.
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

The common SQLite writers retain peer ordering, stable mutation identities and
an atomic import checkpoint. The move-after-join repair is committed at `b5dafd918`;
`7395cf3bb` removes its selection coupling and repeated dependency scans. Public setup and local-only foreground exchange consume these primitives; focused executable evidence is below.
Export reads existing identities rather than minting edits. Import and projection
commit together; fetched Git history is not an import acknowledgement. Crashes before commit leave import retryable.
CLI status separates fetched revisions, retained imports, current eligible-export
changes and pending/unconfirmed/confirmed publication. Linear delivery stays separate;
Desktop peer-status presentation and full composition remain unfinished.

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
not that every record projected. Only the final dependency pass supplies active
errors; an intermediate missing-parent error must not survive a later cycle diagnosis.
Historical published reasons survive resolution. Invalid documents, reused IDs, cross-repository ownership and unexpected
SQL failures still roll back the whole import. The common writer suppresses echo,
advances Task optimistic revisions only on changed planning and calls no execution
writer. Paths stay local. The single `planning_peers.sql` draft still depends on
`local_planning`. Destination binding and membership now live in the same draft. Joining is empty;
export joins the mutation journal to explicit selected-record membership, never to
all repository rows. Selecting a Wave includes its descendants; new descendants
inherit that membership. Imports reserve incoming identities transactionally and
refuse overlap with unselected live records or retained journals. Incoming references cannot
attach selected work to an unselected parent. Conflict settlement is destination
scoped. An explicit canonical user UUID can be provisioned once or recovered on
another store; connecting never invents a per-machine user. Public setup now consumes
these APIs; focused setup evidence is retained below. The common writer retains moves without changing
sharing selection. Exchange now holds affected history instead of refusing the
whole destination; the boundary and remaining proof are below.

## Delete — do not maintain

The prior cut-by-cut inventory and evidence remain at
`b0f2a00702ccdbe1b11c22fa55f472cc649bf8d1:scratch/work-on-another-machine-name.md`,
this heading. Keep these replacements deleted:

- Copied `TaskSource.planning`, issue-derived `TaskId::from_issue`, UUID-v5,
  host callbacks and exclusive adoption fixtures. SSH carries saved identity and
  pushed-code requirements; common planning supplies the target. The cold-worker
  fixture uses dispatch publication/acquisition, never manual import.
- Repository-wide export, alias-addressed transport and selection coupled to moves.
  One destination-scoped hold calculation excludes whole private histories and
  dependents without vetoing independent exchange. Returning to a shared parent
  cannot release private historical references; only explicit selection can.
- Value-only and copied causal-head indexes, full-journal scans per object and
  repeated provider-frontier selection on projection retries. The borrowed object
  view carries winning mutations and eligible provider observations, prepared once
  for unheld objects. Original history stays in the immutable snapshot; latest
  revision groups retain contradictions and acquisition ages.
- Raw Project Markdown mutations, duplicate workflow representation, a SQL
  content parser and the `project_content::capture_content` forwarding wrapper.
  `project_content.rs` owns parsing/persistence; `planning_peers::capture_project_content`
  reads that canonical content and only its three field heads. Import returns before
  reading it, suppressing echo without a redundant Markdown round trip. Migration
  and common writers call the same capture owner; list receipt granularity stays.
- Peer-only scalar/state/comment receipt writers, reconstructed comment provenance,
  identical-body acquisition bypasses and replay of superseded provider revisions.
  Common acquisition owns validation; savepoints isolate valid contradictions and
  retain membership evidence after rollback. Entity revisions never order relationships.
- Project creation/link fields on `project_transitions` and imported synthetic
  transition settlements. The existing Project row now owns those effect columns;
  the peer draft preserves them and drops the transition copies. Common
  `planning_export.rs` still owns capture, attempts and readback. Local transitions,
  activation, transfer membership and selection remain local.
- Machine-local creation sequence cutoffs. Captured local/peer change IDs bound
  acknowledgement; the original parent is retained for private-selection holds.
  Optional creation groups use the same journal, not a second receipt transport.
- `peer_task_creation_insert` and `peer_project_creation_insert`: no writer inserts
  a prepared receipt. `prepare_planning_export` updates an existing row, so the
  update triggers alone capture preparation and attempts; migration still seeds
  retained receipts and imports suppress echo. Readback prepares its two field
  statements and serializes captured IDs once per receipt, not once per field.
- Transition/intent-only export discovery, mapping-only completion shortcuts and
  duplicate status eligibility filters. One `planning_exports` view reads common
  receipt acknowledgement; preparation alone creates missing Task receipts.
- Local-receipt-only attempt eligibility after rejected peer projection. The
  common attempts consult retained projection conflicts, including skipped holds,
  in their write transaction; pending/readback and saves remain independent.
- Sequence-selected Project order intentions and delivery of settled, superseded
  partial moves. Shared views select the original save, while uncertain losing
  effects still hold delivery. Import projects the chosen list after Task rows;
  scalar rank replay cannot overwrite it or advance revisions on reimport.
- Sequence-selected deletion delivery and scalar-clock visibility over a retained
  removal receipt: neither imported arrival nor that clock orders provider evidence.
  Common pending/attempt/status readers retain every unresolved
  removal identity. Tombstone projection alone never prepares or settles a delete.
- Deletion import's scalar `record_value` followed by an overwrite. The common
  deletion owner now upserts the merged captured receipt once, without sampling
  this machine's baseline or visibility. History decoding streams rather than
  collecting another receipt vector. Creation/deletion conflicts use one typed
  Store error, not diagnostic-string matching; malformed input still aborts import.
- Acquisition-side `inherit_project_placement`: provider readback is planning,
  not first local execution. Explicit placement retains its existing owner.
- Task creation receipt bypass after mapping-only import, missing-list readiness
  rejection and required ancestor placement on first local use. Readback reconciles
  captured saves, not later intentions; local execution placement stays local.
- Repository-global status/conflict readers and status rendering before dispatch.
  One destination snapshot retains unknown pending state for malformed journals;
  healthy plans and receipts remain visible. SQL failures still fail the read.
- Duplicate cache-miss acquisition in `pm::resolve_saved_task` and `task::prepare_task`:
  `planning_peer::find_task` acquires only missing planning without placing it.
  Launch freshness and pushed-code checks remain separate.
- Task-attributed sync startup and async waiters owning blocking-worker locks.
  Provider/work-watch lifetimes drive exchange; each worker holds its effect lock
  through readback/status. Short saves commit and release planning locks first.

- Standalone autocommit archive SQL and duplicated issue invalidation/reteam
  bodies. Their common transaction helpers now also acquire peer evidence;
  the existing journal retains original ages, negative facts and Team baselines.
  Scalar projection never turns these facts into list-order or execution authority.
- Non-removal capture exclusion and unconditional freshness clearing on peer scalar
  acquisition. Causal detail acknowledgements replace them in the existing journal;
  receiving-time ordering and a second body/reader are not added.
- Replaying every historical invalidation revision against the cache and duplicate
  head replay loops. The greatest retained revision supplies the same floor;
  one outstanding-head handler runs before and after scalar acquisition. Freshness
  reconciliation reads only the cached revision, not another complete entity body.
- Repeated provider-evidence decoding, validation and removal-age sorting inside
  projection retries. `ObjectChanges` retains typed evidence histories beside
  borrowed order histories; document validation remains the malformed-input owner.
  Peer Team acquisition reuses the common cached-Project reader. Independent
  fact savepoints, causal heads, original ages and sharing holds stay unchanged.
- Parallel order-history/head maps and the second next-delivery selection query.
  Each receipt owns its history and heads together; the common delivery view already
  excludes superseded settled intentions. Projection reuses one membership read.
- The peer validator's requirement to rewrite both sort keys. Common moves between
  priority groups intentionally retain the secondary key; transport keeps that exact
  input rather than inventing an additional write.
- Status's intermediate `(DTO, digest)` vector and duplicated reconnect execution
  readers. Destination rows stream within the same read transaction. One fixture
  reader compares full rows and explicitly selected retained Process IDs; new CLI
  inspection Processes remain outside that comparison.

Test-only `export`/`import` helpers remove repeated successful-call plumbing, not
assertions or failure-path checks. No predecessor path or second planner is added.
Remaining grouped owners, legacy association and acceptance are below; omission
and retained conflicts are not convergence.

## Remaining integration — October 9

Creation discovery/readback exists at `a002e4060`, with status assertions corrected
at `098de8033`. Prepared receipt transport is no longer the frontier; the evidence
below distinguishes public acquisition from storage-only proof.

`84f8fb9d8` deletion receipt composition extends `planning_changes.rs`, not a peer-only
delete writer. Each saved deletion has its own optional journal field keyed by the
common receipt ID. Capture covers saves, attempts, acknowledgement, errors and
explicit active-issue conflicts in the same transaction; the existing peer draft
seeds retained rows. Import unions attempts and settlement for the same baseline,
keeps all losing values, and never infers acknowledgement from a tombstone, mapping
or missing inventory. Creation readback may fill an unattempted null baseline;
it acknowledges creation, not removal. Conflicting baselines or unordered active/
trash outcomes isolate the object and retain its journal.

Review caught local `seq` being treated as deletion recency: a late historical
receipt could hide a newer save. All unresolved deletion identities now remain
visible to the common pending/attempt/status readers; uncertain attempts still
exclude another write. Explicit active evidence retires older concurrent removals
through one visibility reconciler used by local acquisition and peer import.
A removal saved against that active revision survives. A rerun exposed a second
ordering error: the independent visibility scalar could outrank that removal's
receipt. Receipts now capture the original save time in `task_changes`; where
receipts exist, their common reconciler alone projects visibility. The regression
forces the contrary scalar clock rather than relying on timing. Provider-only
removal now uses the common acquisition cut in item 3. Reimport changes no revision.
Focused regressions cover these cases, migration, late errors and full-row equality
for seven execution tables (including populated Sessions, Processes and Workflows),
plus the retained checkout. They do not prove running-provider controls or public
mixed-provider lifetime behavior. Ordering and alternate acquisition are below; composed verification remains
unfinished and mixed-provider exchange stays disabled.

The latest sync includes issue-scoped comments and child-readiness repair.
TESTING.md requires comment acquisition in a different order from Task creation.
Gate still owns the combined admission, cold-worker and provider-recovery candidate;
parent integration does not establish peer composition.
Destination-scoped status/conflicts (`de9470d5a`), common comment acquisition and
receipts (`7e56c01fd`), and semantic Project content (`a518b985b`) are integrated.
Their boundaries are under **Delete — do not maintain** and item 4.

Creation/link discovery and receipts share `planning_export.rs` and
`planning_exports`; mappings and imported transitions never acknowledge effects.
Exact capture/readback proof: `95e81c688:scratch/work-on-another-machine-name.md`,
**Remaining integration**.

At `a002e4060`, the isolated Linux connected-provider fixture passed after importing
unprepared plans before connecting Linear. It exercises public work-watch, Task
edits/status and Project edits/workflow readback against synthetic HTTPS. Lost
creation/link responses, mapping-only evidence and later saves require readback
without repeated writes. `098de8033` then corrected status assertions to read
`sync.changes` directly; execution of that revised Linux fixture remains with gate.
Neither result enables mixed-provider Git exchange or establishes simultaneous
cross-machine creation authority.

The fixture compares complete rows in six tables: Sessions, Workflows, Workflow
moves, Task PRs, placements and Project transitions. Its peer-born records are
unrun. The shared reader now also queries Processes, but this creation case
passes an empty retained-ID list. This proves no fabricated placement or
transition, not populated execution/control preservation. That separate acceptance
remains with the two-store foreground fixture and combined gate.

Executable feedback exposed two distinct presentation/execution boundaries:
- `wave status` rejects imported Waves without Machine placement;
  `WaveSnapshot.machine` is nonoptional in Rust/Swift. Keep imports unplaced.
  The remaining presentation cut must represent absence, not allocate execution
  on reads. The existing public `project workflow show --json` reader exposes
  Project sync receipts independently; it proves no Wave/Desktop acceptance.
- Common provider acquisition called `inherit_project_placement`, allocating
  placement during creation readback. That call is deleted. Explicit local
  creation/placement keeps its owner; acquisition preserves existing placement
  and absence. The public fixture retains the six-table comparison above that
  exposed this failure rather than relaxing it.

Remaining composition, reconciled October 9 against `25cbc0a09`:

`358f7fa08` implements alternate removal/archive/Team acquisition;
`25cbc0a09` prepares typed histories before projection retries. These supersede
the earlier instruction to implement item 3, not its public acceptance.
Ordering (`fcd64901f`, `357090d4b`) and retained-effect eligibility (`08a285872`)
remain integrated. The latest 63 peer-storage passes include the alternate cut;
the public reader change at `30b476328` still awaits gate.
`5bed3a211` integrates main `3a0aa5ca4` (#1510), after common writer `d20c56daf`.
Main adds chat Flow recipes and hierarchical names, not provider frontier transport;
combined launch/Flow verification remains with gate.

1. **Retained-effect eligibility is implemented, with bounded proof.**
   Common field/deletion, creation/link, state and ordering attempt transactions
   consult active projection conflicts. Saves, pending reads and acquisition do
   not. Skipped sharing-held projections retain conflicts even after selection;
   only successful import releases them after incorporating the retained receipts.
   `08a285872` records passing storage and public retained-effect regressions:
   a stale frontier carrying an uncertain deletion prevents another delivery,
   while independent title/comment acquisition and populated execution survive.
   The public fixture imports directly before connecting its synthetic provider;
   it proves neither mixed-provider Git transport nor running-control preservation.
   `30b476328` changed its shared snapshot reader without rerunning that Linux
   public case; the combined gate must cover it. Exact earlier proof and rationale:
   `08a285872:scratch/work-on-another-machine-name.md`, **Remaining integration**
   and **Acceptance for review**.
2. **Ordering receipt transport is implemented with focused proof.**
   `planning_order.rs` imports each captured move's baseline, desired list,
   exact effects, acknowledgement and conflicts through the existing journal.
   The first mutation's logical clock plus receipt ID selects the saved intention;
   attempt/readback clocks and receiving-machine sequence never select it.
   Common views drive pending, delivery and status. Any unresolved losing effect
   still prevents another write; settled partial progress cannot revive a losing
   desired order. Only complete-list acquisition recognizes progress and rebases
   unattempted saves. Scalar ranks, detail reads and import checkpoints settle nothing.
   Lists include private-history references in the existing sharing holds.
   Divergent effect chains or unordered baseline heads isolate projection.
   The two-store common-delivery fixture covers lost replies, cold empty-list
   rejection, reopening and a later save. Compression exposed rejection of valid
   primary-key-only moves: the common writer omits `sortOrder` when moving between
   priority groups. Transport now retains that exact input. A regression failed
   on export before repair and exercises peer readback without another write.
   Migration, selection and competing-effect proofs remain distinct from public
   mixed-provider acceptance.
3. **Alternate removal/archive/Team acquisition is implemented.**
   Optional provider-evidence fields use the existing peer journal, separate from
   entity provenance and saved delivery receipts. Removal/archive evidence commits
   independently of rejected stale scalar projection; confirmed Teams retain their
   baseline and original acquisition age, without ordering Initiatives or complete
   lists. Concurrent contradictory Team confirmations isolate the object. Retained
   membership uncertainty still needs a fresh common confirmation, not an old
   imported receipt. Migration seeds retained negative evidence with unknown age;
   it does not invent historical Team confirmations. Non-removal notifications
   use the causal invalidation cut in item 4, not durable removal evidence.
   Each fact has its own savepoint:
   conflicting Teams cannot roll back an independently confirmed archive.
   Review/tests repaired unchanged removal writes and reteam's stale unversioned
   entity conflict; newer entity values and their age survive confirmed Teams.
   Repeat-import checks use the same Git revision: changing the import checkpoint
   legitimately notifies planning readers and is not projection churn.
   Focused fixtures cover source-only removal against stale detail, archive without
   a delete receipt, reteam beside newer entity facts, conflicting confirmations,
   malformed evidence, migration and populated execution. The Team fixture retains
   an uncertain order without acquiring a list; the earlier order fixture separately
   rejects a cold empty list. Neither proves partial-list acquisition composed with
   alternate provider evidence. That case and public acquisition remain gate coverage,
   not reasons to weaken acceptance or claim mixed-provider lifetime behavior.
4. **Replay-safe invalidation is implemented with focused proof.**
   `provider_invalidation` shares the existing journal's causal heads between
   notifications and successful detail acknowledgements. Detail retires only the
   notices it observed; unseen concurrent notices remain outstanding. Known revision
   floors survive acknowledgement. Neither arrival time nor the receipt's clock
   becomes a provider revision. Import clears freshness only after accepting the
   corresponding entity frontier; scalar/list replay no longer clears it alone.
   The acknowledgement carries identity, revision and original age, not another
   entity body or parent mapping. A cache-only detail without transported scalar
   facts cannot refresh another machine's older body; the fixture distinguishes
   that from acquisition with confirmed Wave ownership. Null-detail invalidation
   uses the same capture path; migration retains unknown ages and known floors.
   Cold import reapplies outstanding notices after creating the cache. Each fact's
   existing savepoint and private-selection holds remain. Tests cover delayed replay,
   genuinely new and concurrent notifications, equal-body detail, null-detail
   invalidation, populated execution, migration and private-history withholding.
   Review removed unconditional scalar freshness clearing and an unnecessary second
   detail body/reader. A marker-only import also exposed a fresh but older cache;
   the detail's actual revision now fences it through common invalidation, without
   fabricating a revision or accepting a stale local read. Repeated unchanged detail
   does not mint another acknowledgement. Public composition remains gate work,
   not mixed activation.
5. **Legacy association and presentation remain implementation work.** Divergent
   existing IDs retain history; unplaced Waves need nullable Rust/Swift Machine
   presentation, not placement on read. Desktop still needs selected-plan,
   authorship/assignee, pending/conflict and losing-edit/recovery presentation.

Mixed-provider exchange remains disabled until composition is complete. Local-only
exchange and private-selection holds remain intact. Combined verification belongs
to gate; publication still requires the acceptance below, with no landing authority.
Earlier cut-by-cut evidence: `3f7d9f2da:scratch/work-on-another-machine-name.md`,
**Remaining integration**.

October 8 executable repairs (registry error conversion, SQLite bindings and
required local Task defaults) remain at `08a285872:scratch/work-on-another-machine-name.md`,
**Remaining integration**. Imported planning stays unplaced; those focused results
supersede Rust-unavailability claims, not composed acceptance.

Earlier acquisition-age/readiness findings and the public work-watch evidence:
`fcd64901f:scratch/work-on-another-machine-name.md`, **Remaining integration**.
They establish neither connected Linear/Git composition nor real SSH.

Earlier parent-integration results and the synthetic-state-settlement removal:
`9b59e9b71:scratch/work-on-another-machine-name.md`, **Remaining integration**.
They establish no combined peer acceptance; observed facts, not errors, settle effects.

Source reconciliation at `4e3cb2160` supersedes the earlier no-production-caller
finding: `6c0f2256f` connects exchange and `4e3cb2160` isolates destination attempts.
The repository-scoped `linear_observe::PlanningSync` now follows interactive and
headless provider lifetimes, including taskless starts and native reconnect, plus
repository work-watch connections. Task creation/edit/comment/refile/completion,
cancellation/deletion and Project content/name/workflow edits save first, then
attempt peer publication (which acquires first). These short commands have no
resident lifetime. Independent Git
acquisition and publication branches use blocking workers; a publication worker
retains its effect lock through readback/status even after waiter cancellation.
Saves and acquisition do not take that lock. Machine dispatch checks pushed code,
then publishes selected planning; Task resolution acquires before looking up a cold
identity. Target-only Tasks still work without a local repository.

Publication fetches/imports before every decision, unions only the current eligible
export with remote history, and never republishes private held edits from a retained
local Git document. An unchanged remote covering the export settles by readback
without another commit. Uncertainty is saved before push; later local edits are
compared with the attempted snapshot digest. Fetched, imported and publication
revisions remain separate. The public work-watch case passes in isolated Linux;
other composed cases remain gate acceptance.
Linear-connected repositories still do not exchange: the removal/archive/Team
storage cut exists, but legacy association, presentation and combined public acceptance (including
invalidation) remain. The diagnostic is temporary, not product policy.

1. Public `lf planning key/connect/use/select/status` now provisions/recover keys,
   pins an empty binding, selects exact Wave IDs and displays retained imports/holds.
   `use` routes only future root Waves; descendants inherit saved membership. Joining
   and switching never enroll existing work. Root routing and imported-root isolation
   live in the existing peer draft. Status omits credential-bearing endpoints.
   Focused setup and public creation/edit checks pass in isolated Linux.
   Setup never contacts the remote. Background exchange consumes the selection;
   full acceptance and final presentation remain.
   Holds now have CLI presentation. Selecting referenced Waves explicitly may release
   retained private history; returning to a selected parent cannot. Recovery/losing-edit
   UX still needs review, not a new membership model.
2. Execute the existing foreground composition regressions rather than rebuilding
   their callers. Public `planning_foreground_tests` now uses
   Task create/edit/comment/move commands on both stores rather than SQL mutations.
   It covers cold comment acquisition without placement, distinct same-title
   identities, offline saves, both comment authorships, completion, duplicate-free
   delivery and reconnect while acquisition bypasses an outbound lock. The receiver's
   retained Session, Process, Workflow, PR and checkout must remain unchanged.
   A separate two-store test runs taskless terminal/headless stand-in providers,
   requiring reconnect convergence without another turn or work-watch connection
   and exactly one provider launch per Session. It selects destinations after
   launch, so startup cannot freeze an empty selection. The work-watch test passes
   in isolated Linux; the provider-lifetime test remains unexecuted.
   Native reconnect uses the same new lifetime, but has no composed peer regression
   yet. The taskless test forces `-i` with null stdin and a stand-in provider: it
   proves neither a native PTY nor reconnect, even when it passes. Short commands await one bounded transport attempt after
   committing; unavailable or busy destinations stay pending. Cached Task mutations
   do not wait for acquisition before saving. Cold completion/cancellation/deletion
   now use the same planning resolver as edits/comments, retaining historical branch
   selectors for lifecycle commands and never allocating execution during lookup.
   Invalid fetched documents must not advance import/publication. The existing
   recovery test covers lost-receipt readback without a new commit and import after
   fetch-only interruption. Public standalone Project/Wave lifecycle coverage and
   Desktop selected-plan/status, authorship and assignee presentation remain.
   No automatic turn/Flow retry exists.

3. Resolve divergent legacy IDs through explicit provider associations without
   renumbering stored Work. Conflict isolation is implemented with new regressions
   for duplicate mappings, dependent comments, retained Sessions, selection and
   idempotent retry; the focused storage suite passes. CLI `planning status`
   already reads projection conflicts; Desktop still needs that presentation.
   A durable import checkpoint is not completed projection.
   Dispatch and status isolate malformed journals: `pending_local: null` plus a
   sanitized error, with retained conflicts and fetch/import/publication receipts.
   Healthy plans and sharing holds remain visible; SQL failures still fail the read.
   Public JSON/text recovery, exchange isolation and DTO regressions are authored;
   executable acceptance remains with capable gate/CI. Desktop needs the same
   presentation. Status never repairs a journal or equates retention with convergence.
4. Complete optional Linear composition. Task title/brief/assignee/membership and
   Project name/summary/workflow/status use the common scalar receipt writer in the
   import savepoint. Stable peer mutation IDs supply delivery identity. The latest
   causal Linear predecessor now supplies both value and provider revision; imported
   winners retain losing values, attempted flags and errors without acknowledgement.

   Common scalar, state, comment, content, creation/link, deletion and order
   receipt composition is implemented. Provider entity observations retain their
   original bodies, revisions and ages; same-revision contradictions isolate an
   object while malformed input aborts import. Readback does not acknowledge a
   mapping or execution. Full implementation/proof details remain at
   `5624151a3:scratch/work-on-another-machine-name.md`, this item. The earlier
   checks are storage evidence, not connected Git/Linear lifetime acceptance.

   Removal/archive/reteam now use common acquisition with the focused storage
   proof in Remaining integration item 3. Gate still owns public alternate
   acquisition, complete-list ordering and comment composition, not only detail
   reads. Legacy association, Desktop status and public invalidation verification remain
   before mixed-provider exchange and removal of its temporary diagnostic. This
   cut cannot repair LOO-406's unseen provider-write race or supply Linear
   compare-and-swap semantics.

5. Execute the public cold-worker regression: an absent issue selector must acquire
   automatically, run pushed code, then reuse the same identity/checkout by full ID.
   Its manual peer import is removed. Broader local-born creation, legacy association,
   real SSH and installed acceptance remain; failed acquisition preserves saved plans
   and the existing missing-synchronization diagnostic.
6. Run the focused Rust checks, then gate's complete acceptance and the final
   walkthrough before republishing #1491. Parent `planning_reconnect_tests` now
   exercises public work-watch and Flow commands against synthetic Linear HTTPS;
   `TESTING.md` assigns it to Linux because macOS ignores the fixture CA setting.
   That public lifetime coverage is reusable without a second Linear harness;
   it neither exercises Git peers nor replaces the two-store/disposable-ref cases.
   Ordinary checkpoint PR CI skips product suites while scratch is present
   (`TESTING.md`); publication alone cannot supply the deferred checks. A capable
   isolated gate must execute the peer candidate, or CI must demonstrably run its
   suites. No scratch deletion, landing or installation is authorized just to obtain
   a green check. Headless Desktop tests remain distinct from review judgment.
   No publication, landing or installation has occurred in this implementation pass.

## Committed integration boundary — 2026-10-09

Jack Heart requested stacking on LOO-406 and continuing pursue. `225b8c95f`
integrates main's LOO-406 cut `d20c56daf` (#1503). Its tree is identical
to `97bba1869`: this sync added ancestry, not another implementation or test change.
Earlier parent integrations, restored scratch checkpoint `1ad5e1267` and retained
stash provenance: `225b8c95f:scratch/work-on-another-machine-name.md`, this heading.
The common cut `84664e661` removed `PlanningAuthority`, personal-plan storage and
split writers. Creation/export, ordering, common sync presentation and foreground
lifetime owners are integrated; LOO-412's remaining peer composition is independent
work, not a missing parent dependency or permission to add another planner.

Jack's destination policy uses a stable user key across machines, never Git display
names or per-machine random users. Ref separation is not access control: private
planning requires a controlled remote. Joining must preserve existing plans without
silently merging or uploading them. Synthetic fixtures remain the only authorized
publication target. LOO-406's unseen-Linear-write race is contrary provider-atomicity
evidence, not a missing peer-sync decision. Public acquisition, Linear receipt composition, legacy association
and the remaining integration above are required before review-only publication.

## Acceptance for review

Acceptance requires CLIs built from this worktree, isolated stores, synthetic plans
and disposable remotes. Installed-release behavior remains separate. Release's child memory
records a recovery path missed by lower-level fixtures and a successful failure
report mistaken for operation success. The same lessons require public command
coverage here, and status that distinguishes retained conflicts from convergence.
October 9 reconciliation reread Release's GOAL.md and full MEMORY.md; filesystem
inspection found no other immediate child memory. Its operation-entry and
result-accounting lessons remain in parent memory: storage readback cannot prove
foreground acquisition, and a successful report cannot replace operation success.

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
   changing a remote alias must not publish to a newly pointed destination. Local
   and provider-origin moves after joining must preserve selection isolation and
   independent exchange without implicitly exporting an unselected parent.
5. Public `--machine` launches by issue/local selector and label/ID on a blank
   worker see pushed code and reuse placement. Cover local-born identity,
   divergent legacy IDs, unpushed source, missing branch and behind/dirty target.
6. Exercise local-only planning and contained optional Linear pending sync through
   the common writer. A local bare remote does not prove hosted custom-ref policy;
   that requires an explicitly selected disposable hosting repository.
7. Replace PR copy and create a walkthrough of final behavior and evidence;
   publish #1491 for Jack Heart's review and stop without landing.

The integrated Store now validates Workflow source as well as its name. The
preservation fixture covers both failures; the earlier claim that parsing belonged
only to `ops::project::workflow` is superseded. Creation-status checks read the
public `sync.changes` field directly on Task status and Project workflow show,
without recursive JSON searching or guessing whether stdout contains JSON.
Earlier fixture evidence: `a002e4060:scratch/work-on-another-machine-name.md`,
**Acceptance for review**.

Fixture repairs remain at `cdec83fe8:scratch/work-on-another-machine-name.md`,
this heading; earlier Project-content/public work-watch evidence remains at
`b0f2a00702ccdbe1b11c22fa55f472cc649bf8d1:scratch/work-on-another-machine-name.md`,
this heading. Public status/DTO, cold-worker and provider-lifetime execution remain
with gate, alongside Desktop and mixed-provider acceptance.

Prior storage/build/Clippy/fmt and public recovery proofs, with exact candidates
and limitations: `9b59e9b71:scratch/work-on-another-machine-name.md`,
**Acceptance for review**. They do not establish this ordering cut, public
combined behavior or Desktop acceptance.

Check (October 9 compress): `cargo test -p loopflow --lib peer_invalidation --no-run` builds; `scripts/test_network.py <lib-test>` passes `peer_invalidation` (2), `peer_provider_removal_and_archive_precede_stale_detail_without_losing_work` and `provider_move_to_unselected_project_preserves_execution_and_sibling_exchange` (4 total); `cargo fmt --check`, `git diff --check` and `cargo clippy --all-targets -- -D warnings` pass. Combined public, partial-list and Desktop checks remain with gate.

Earlier SQL proofs, macOS startup limitations and the retained historical Flow
without a recorded exit remain at
`eeb98aa89:scratch/work-on-another-machine-name.md`, **Acceptance for review**.
No provider death, replacement authority or installed acceptance follows from those
observations. Parent-sync timeout evidence remains at
`a6cb48664:scratch/work-on-another-machine-name.md`; it supplies no test verdict.
LOO-406's landing authorization does not extend LOO-412's review-only boundary.
