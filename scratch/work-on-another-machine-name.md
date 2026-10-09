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
Desktop streams destination status; per-Work presentation and full composition remain unfinished.

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

- Earlier transport/selection/content deletions remain at
  `b4a91632e:scratch/work-on-another-machine-name.md`, this heading: copied
  planning, issue-derived IDs, host callbacks, repository-global export,
  alias-addressed routing, move-coupled selection and duplicate content parsers.
  Preserve saved identity, pushed-code checks, private-history holds, causal
  journals and the common semantic-content writer.
- Evidence-only mapping validation and the duplicate-mapping fixture's raw SQL
  unmapping shortcut. One import check owns scalar/evidence mapping preservation;
  retain both IDs, mutation history, effects and independent projection.
- Peer-only scalar/state/comment receipt writers, reconstructed comment provenance,
  identical-body acquisition bypasses and replay of superseded provider revisions.
  Common acquisition owns validation; savepoints isolate valid contradictions and
  retain membership evidence after rollback. Entity revisions never order relationships.
- Creation/link effect columns on `project_transitions`, Project rows and
  `task_creation_intents`. The peer draft migrates released receipts directly to
  common `planning_creations`, keyed by kind/original Work ID with a separate local
  Task/Project foreign key. Task request idempotence stays on its existing owner.
  Common `planning_export.rs` owns capture, attempts and readback; transitions,
  activation, membership and execution stay local.
- Peer import's synthetic `task_creation_intents` insertion. Only an actual local
  creation request owns idempotence; the common receipt already retains its original
  parent and model. Retain released local requests. Creation validation returns its
  decoded receipt, and retained receipt reads use typed columns rather than rebuilding
  wire JSON in SQL. Check distinct origin/projection conflicts without querying the
  same identity twice. Existing moved-parent and empty-local-history fixtures cover
  the surviving owner; execution of these changes remains deferred below.
- Machine-local creation sequence cutoffs. Captured local/peer change IDs bound
  acknowledgement; the original parent is retained for private-selection holds.
  Optional creation groups use the same journal, not a second receipt transport.
- Separate Task/Project creation capture triggers. Common receipt insert/update
  triggers retain the original Work ID; changing local projection alone creates no
  journal mutation or sharing membership. Migration seeds retained effects and import
  suppresses echo. Readback retains captured IDs, not local sequence positions.
- Transition/intent-only export discovery, mapping-only completion shortcuts and
  duplicate status eligibility filters. One `planning_exports` view reads common
  receipt acknowledgement; preparation alone creates missing Task receipts.
- Local-receipt/Work-ID-only attempt eligibility after rejected peer projection.
  Common attempts consult retained conflicts, including skipped holds and another
  ID claiming the same provider object, in their write transaction. Losing mappings
  and creation inputs survive; pending/readback and saves remain independent.
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

Earlier decoding, ordering, Desktop and test-helper reductions:
`39dba32c1:scratch/work-on-another-machine-name.md`, **Delete — do not maintain**.
Keep independent savepoints, original ages, exact order inputs, post-update effect
eligibility and Desktop scope/generation fences. The derived
`planning_peer_provider_claims` view owns retained mapping/creation predicates;
issue-name fallback records no association. Item 3 remains unfinished.

Delete cloned head-value collections in `FieldHistory`: each decoded value now
carries its head marker, with borrowed iteration for acquisition and order import.
Scalar SQL projection uses `PlanningKind::fields`, not a second exclusion list
of creation/deletion/order/evidence protocols. Receipt writers, all retained
mutations and association holds remain unchanged; no joint projection is added.

Delete the creation-origin fixture's direct `import_peer_receipts`/`attach_in`
shortcut: ordinary association/import and common acquisition now exercise that
boundary. Keep both origin IDs, captured inputs, uncertainty and private membership.

Association and import share one conflict writer and one skipped-projection
receipt. Delete diagnostic-text branching between association and sharing holds:
both retain incomplete effects until successful import; status derives their
specific current reasons. One object scan covers repository and association
holds. Historical conflict rows survive; no correspondence or mixed-provider
authority is added. The association fixture checks one current explanation for
each Task/Project after reversed and repeated imports.

## Remaining integration — October 9

Receipt origins and full-ID correspondence exist. Ordinary import now projects
associated creation receipts onto their local owner without releasing holds
(`39dba32c1`).
**Joint projection and association recovery remain unfinished**; item 3 owns
the causal, relationship and receipt composition before lifting those holds.

Destination-level Desktop status has focused headless proof; per-Work membership,
authorship/assignees and losing-edit recovery remain unfinished. Public Git/Linear
composition also remains. The temporary mixed-provider refusal in
`ops/planning_peer.rs` stays enabled; retained receipts are not an end-to-end result.

Deletion composition and its counterexamples remain at
`506efbbfe:scratch/work-on-another-machine-name.md`, **Remaining integration**.
The common receipt owner retains all unresolved identities, original save times,
uncertain effects and explicit active/trash conflicts; scalar clocks, mappings and
absence cannot settle removal. Focused storage proofs preserve populated execution,
not running controls or public mixed-provider lifetime behavior.

Desktop status uses one repository-scoped Work part, independent of roadmap
availability. It displays future-Wave routing, membership counts, publication,
fetch/import receipts and holds; failures retain last-good readings and old
scope/Machine frames are fenced. One-shot transports share the reader. No setup
action, new poller, schema or transport activation is added. Git publication and
Linear delivery remain independent. Detailed proof: `c20e4ad13`.

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
- Wave list/status/roadmap now expose nullable `WaveSnapshot.machine` in Rust
  and Swift; text and Desktop show “unplaced.” Reads retain imported planning
  without allocating placement. Project sync receipts remain independently visible.
- Common provider acquisition called `inherit_project_placement`, allocating
  placement during creation readback. That call is deleted. Explicit local
  creation/placement keeps its owner; acquisition preserves existing placement
  and absence. The public fixture retains the six-table comparison above that
  exposed this failure rather than relaxing it.

### Implemented boundaries, not remaining implementation

The detailed ordering, alternate-acquisition and invalidation proofs remain at
`c6874c01b:scratch/work-on-another-machine-name.md`, **Remaining integration**.
Keep their counterexamples and acceptance limits; no second implementation is needed:

- **Effect eligibility (`08a285872`).** Common field/deletion, creation/link,
  state and order attempts consult retained projection conflicts in the attempt
  transaction. Saves and acquisition remain independent. Selection releases no
  effect: successful import must incorporate retained receipts first. The public
  synthetic-provider case preserves populated execution, but imports directly;
  it proves neither mixed Git transport nor running controls. Its shared reader
  changed at `30b476328` without rerunning that Linux case.
- **Ordering (`fcd64901f`, `357090d4b`).** Common receipt identities retain exact
  input, uncertain losing effects and causal baseline heads. The first mutation's
  clock selects the intention, never an attempt/readback or arrival sequence.
  Only complete-list acquisition settles progress and rebases unattempted saves;
  detail reads and scalar ranks do not. Cold empty/partial lists cannot discard
  captured members. Primary-key-only moves omit `sortOrder` and remain valid.
  Import projects pending lists after Task rows; repeat import does not churn rank.
- **Removal/archive/Teams (`358f7fa08`, `25cbc0a09`).** Independent savepoints
  commit negative facts before scalar projection; conflicting Teams cannot roll
  back an archive. Causal heads retain Team/Initiative baselines and original ages;
  newer entity facts survive reteam. Migration retains unknown negative ages and
  invents no historical confirmation. Unknown age never rewrites removal time.
  The 63 storage passes at `25cbc0a09` predate invalidation. Partial-list ordering
  composed with alternate evidence and public acquisition remain gate coverage.
- **Invalidation (`4acd37ec8`, `6ccbbabe3`).** Notification/detail heads retain
  causal acknowledgement and revision floors. Only observed notices retire;
  unseen concurrent notices remain invalid. Scalar/list replay cannot restore
  freshness. Detail carries identity/revision/age, not a second body or mapping,
  and refreshes only an accepted matching-or-newer frontier. Cold import reapplies
  notices; null detail and unknown-age migration use the same capture path.
  Marker-only import exposed an older cache: common invalidation now fences it
  with the real detail revision. The 68-test pass at `e08dc9312` covers storage,
  not public mixed-provider acceptance or the later receipt-origin reduction.
- **Unplaced Wave/status (`60d113a70`, `c20e4ad13`).** Nullable placement and
  destination-level Desktop status have focused headless proof. Public reads
  preserve retained Sessions, Processes, Workflow and placements; no live control,
  SSH or installed acceptance follows. Per-Work selection, authorship/assignees
  and losing-edit recovery remain implementation work.

`5bed3a211` integrated main `3a0aa5ca4` (#1510), after common writer `d20c56daf`;
`ce1750393` integrated #1512. These add chat recipes/hierarchical names and driver
recovery, not provider-frontier acceptance. Combined launch/Flow verification
remains with gate. The single remaining-work list below owns association,
presentation and public composition; retained conflicts are not convergence.

Mixed-provider exchange remains disabled until composition is complete. Local-only
exchange and private-selection holds remain intact. Combined verification belongs
to gate; publication still requires the acceptance below, with no landing authority.
Earlier cut-by-cut evidence: `3f7d9f2da:scratch/work-on-another-machine-name.md`,
**Remaining integration**.

Earlier registry, SQLite-binding and Task-default repairs:
`08a285872:scratch/work-on-another-machine-name.md`, **Remaining integration**.
Imported planning stays unplaced; focused results prove no composed acceptance.

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
   Native reconnect starts the same foreground sync owner, but has no composed
   peer regression yet; the main integration below distinguishes live handoff from
   resume after driver death. The taskless test forces `-i` with null stdin and a
   stand-in provider: it proves neither a native PTY nor reconnect, even when it
   passes. Short commands await one bounded transport attempt after
   committing; unavailable or busy destinations stay pending. Cached Task mutations
   do not wait for acquisition before saving. Cold completion/cancellation/deletion
   now use the same planning resolver as edits/comments, retaining historical branch
   selectors for lifecycle commands and never allocating execution during lookup.
   Invalid fetched documents must not advance import/publication. The existing
   recovery test covers lost-receipt readback without a new commit and import after
   fetch-only interruption. Public standalone Project/Wave lifecycle coverage and
   Desktop per-Work selection, authorship and assignee presentation remain.
   No automatic turn/Flow retry exists.

3. Resolve divergent legacy IDs through explicit provider associations without
   renumbering stored Work. **October 9 counterexample:** clearing the legacy
   mapping is not association. `planning_exports` then treats that retained Task
   or Project as a new provider creation; the old issue identifier also remains
   in selector lookup. Pending effects still belong to the legacy ID. The old
   duplicate-mapping test's raw SQL shortcut established projection only, not
   safe recovery, and is removed. An explicit association must preserve both IDs,
   local execution lookup, common effect ownership, uncertain receipts and private
   selection. Neither transferring a mapping nor merging execution is an accepted
   implementation. The common receipt-origin cut below resolves storage multiplicity.
   Explicit correspondence/lookup now exists; joint projection and completed
   recovery do not.
   Mapping validation precedes scalar and independent-evidence acquisition.
   Common effect attempts consult cross-ID conflicts against retained provider
   claims, including losing mappings and creation inputs; nulling a mapping
   cannot release them. Saves/acquisition remain independent. These implemented
   refusals are not association recovery; exact history:
   `f2a5c94e8:scratch/work-on-another-machine-name.md`, item 3.

   **Receipt-origin cut — October 9, not a new product decision.**
   Common `planning_creations` now holds each original Task/Project creation and
   attachment operation under `(kind, origin_id)`, separately from its local
   Task/Project foreign key. Capture validates the original UUID/model unchanged.
   Import merges attempts/acknowledgements per origin, rejects silent reassignment,
   and retains both operations when two origins target one projection. Discovery
   groups by projection for locking, then delivers/readbacks each origin separately;
   status shows original receipt IDs. No Project transition or parallel peer queue
   is added. Released Task/transition effects migrate directly into this final
   draft; the obsolete columns and per-kind capture paths are deleted. Review caught
   pre-capture diagnostic loss: nullable captured input now preserves discovery
   failures through migration and status without inventing an attempt.

   **October 9 receipt composition:** ordinary import consumes explicit
   correspondence and projects each creation origin through the existing common
   receipt writer, independently of the held scalar projection. Savepoints retain
   contradictory operations without replacing receipts or stopping other objects.
   Import suppression avoids echo; neither private selection nor execution moves.
   No reassignment API is needed: correspondence already refuses physical incoming
   rows. Lookup also rejects contradictory incoming scalar mappings.

   The former direct-helper fixture is replaced by
   `correspondence_import_retains_creation_origins_and_exact_readback`: both kinds,
   reversed/repeated imports, retained uncertainty and captured inputs, private
   membership, populated execution, and ordinary Store acquisition settling only
   the exact origin. It also covers later contradictory mappings. Execution is
   deferred below; this is not public Git/HTTPS or joint scalar acceptance.
   Helper evidence: `de3c84b08`, this item. Readback of a
   different created provider object cannot transfer mappings or settle receipts.

   `planning associate <incoming-id> --with <local-id> --linear <provider-id>`
   now records a machine-local correspondence using the incoming journal's
   unambiguous scalar mapping and the existing local row's same exact mapping.
   It refuses redirecting an existing physical row. The common Task resolver and
   Project selector resolve associated full IDs only; direct identity readers
   remain exact. Membership, captured effects and execution foreign keys do not
   change. Lookup rechecks mapping/repository, and association/reimport keeps an
   explicit projection hold rather than enabling partial effects. This temporary
   hold is incomplete implementation, not Jack Heart's final policy.

   The Store correspondence fixture imports older/newer snapshots in both orders,
   repeats association and resolves both IDs. It asserts unchanged private planning,
   local uncertain field receipts and populated execution; it does not project a
   cross-origin winner. The CLI fixture seeds/imports through Store APIs, then runs
   public association and Task status (not Git acquisition or provider readback).
   Its corrected `execution.task_id` assertion awaits rerun; `381c88fec`'s revised
   Store hold assertions remain gate-deferred.

   October 9 source reconciliation at `f2a5c94e8` leaves this implementation order:
   - Replace `insert_and_project`'s incoming-ID-only projection with deterministic
     planning projection across origins, preserving mutation IDs,
     causal heads, baselines and losers rather than last-projected-ID wins.
     Concrete causal constraint: `PlanningSnapshot::validate` currently requires
     each parent to have the same object ID, and common capture reads only that
     ID's heads. Grouping current winners alone would keep an observed peer Linear
     head concurrent with a later local save. Resolve cross-origin observation in
     the journal before enabling projection; a higher local clock alone is wrong.
     `linear_predecessor` follows those same parents for delivery baselines.
     Local correspondence alone is not portable causal evidence: exported saves
     must retain valid predecessors without publishing an unselected origin.
     Keep the journal, head index and import validator consistent; merely relaxing
     the same-object check would accept unrelated predecessors.
   - Resolve planning references as well as the object being projected. Current
     `insert_and_project`/`project_comment` and post-pass Wave selection/order
     projection use incoming IDs directly. Associated parents may have no physical
     incoming row: Task membership, comment ownership and order members must resolve
     to the local planning owner without rewriting journal values, captured effect
     inputs or execution foreign keys. Association alone changes no Wave selection.
     Import coverage must include relationships, not only full-ID lookup.
   - Compose non-creation receipts against the resolved local owner.
     `project_delivery_fields`, deletion import and order import still construct
     owners from the incoming ID. Creation-origin retention does not repair these
     paths. Preserve field/state mutation identities, deletion/order receipt IDs,
     original inputs, baselines and uncertain losing attempts; relationship
     resolution changes the local projection, not captured delivery inputs.
   - Independent private selection: associating private B with selected A cannot
     enroll B, its comments, losing edits or private parent references. Public
     recovery must explain the resulting hold.

   Acceptance must enter through that correspondence operation and ordinary peer
   import/lookup, not substitute helper calls for association. For both Tasks and
   Projects, reversed import order and repeat association must retain the same
   planning winners, both IDs, losing edits, uncertain origins, populated execution
   and private-selection holds. Public provider readback must settle only its exact
   origin; lookup by either associated ID must preserve local execution ownership.
   After observing a peer Linear winner, save locally and exchange again: the
   receiver must retain the observed predecessor and delivery baseline, rather
   than treating the save as concurrent or exporting private history.

   Earlier refusal/68-test coverage and limits:
   `381c88fec:scratch/work-on-another-machine-name.md`, item 3. Private ancestry,
   populated execution and uncertainty survive rejected mappings; no joint
   projection or running-control proof follows.
   Field/deletion, creation/link, state and ordering attempts share
   `require_projected_effects`; composed exact-origin readback remains required.

   CLI/Desktop share destination-scoped conflicts. Malformed journals report
   `pending_local: null` and a sanitized error while preserving receipts and healthy
   destinations; SQL failures still fail the read. Public JSON/text recovery,
   exchange-isolation and DTO checks remain gate-owned; Desktop has focused
   headless proof. Status neither repairs journals nor equates retention with convergence.
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

   Removal/archive/reteam use common acquisition with the focused storage
   proof under **Implemented boundaries, not remaining implementation**. Gate still owns public alternate
   acquisition, complete-list ordering and comment composition, not only detail
   reads. Legacy association, richer Desktop sharing/recovery UI and public invalidation verification remain
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

## Committed integration boundary — 2026-10-09

Jack Heart requested stacking on LOO-406 and continuing pursue. The parent is
now integrated into main; this PR is rooted on main, not waiting on a separate
stack parent. `225b8c95f`
integrates main's LOO-406 cut `d20c56daf` (#1503). Its tree is identical
to `97bba1869`: this sync added ancestry, not another implementation or test change.
Earlier parent integrations, restored scratch checkpoint `1ad5e1267` and retained
stash provenance: `225b8c95f:scratch/work-on-another-machine-name.md`, this heading.
The common cut `84664e661` removed `PlanningAuthority`, personal-plan storage and
split writers. Creation/export, ordering, common sync presentation and foreground
lifetime owners are integrated; LOO-412's remaining peer composition is independent
work, not a missing parent dependency or permission to add another planner.

`ce1750393` integrates main `3e1e6245c` (#1512). Local cleanup now consults
unfinished Process evidence, not pending turns or reservations. Codex/OpenCode
lifelines follow live drivers; resume after a dead driver ends the exact abandoned
engine and starts a fresh engine on the saved native thread. Live connect still
hands off the engine. This supersedes surviving-engine recovery expectations,
not planning's exclusion of execution or the prohibition on automatic turn retry.
The existing taskless peer fixture uses a Claude stand-in and proves neither
embedded-engine handoff nor dead-driver resume. Combined acceptance still needs
planning exchange during a live connection and explicit native resume, with no
extra provider launch caused by exchange; legitimate local resume is not an
imported execution mutation. No engine-lifecycle code is added to LOO-412.

Readback of historical Flow `93d4d4f6-4723-4027-951b-b3aee2e696a2` still leaves
step `e94edd38-2a26-4047-9650-41a65ab814f7` without completion or exit evidence.
The new local recovery implementation does not settle that history.

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
inspection found no other immediate child memory. Its operation-entry and result
lessons remain in parent memory: storage readback cannot prove foreground
acquisition, and a successful report cannot replace operation success.

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

Check: `git diff --check` passes (prose only); preceding `cargo fmt`/`cargo clippy --all-targets -- -D warnings` passed, but resource recovery did not meet the build reserve; build, focused ordering/provider-evidence/correspondence execution and combined acceptance remain with capable gate.

Earlier SQL proofs, macOS startup limitations and the retained historical Flow
without a recorded exit remain at
`eeb98aa89:scratch/work-on-another-machine-name.md`, **Acceptance for review**.
No provider death, replacement authority or installed acceptance follows from those
observations. Parent-sync timeout evidence remains at
`a6cb48664:scratch/work-on-another-machine-name.md`; it supplies no test verdict.
LOO-406's landing authorization does not extend LOO-412's review-only boundary.
