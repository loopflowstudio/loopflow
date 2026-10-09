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
Desktop streams destination and per-Work recovery status; public composition remains unfinished.

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
writer. Paths stay local. The single `planning_peers.sql` draft depends on #1499’s
`optional_task_pr`, which depends on `local_planning`. Destination binding and membership now live in the same draft. Joining is empty;
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

- Earlier transport, selection, content, mapping and receipt deletions are preserved
  at `8a6f8f807:scratch/work-on-another-machine-name.md`, **Delete — do not maintain**.
  Keep copied planning, issue-derived IDs, host callbacks, repository-global export,
  alias-addressed routing, move-coupled selection and duplicate parsers deleted.
  Common acquisition and receipt writers own provider facts and effects; journal
  origins, exact inputs, causal baselines, uncertain losers and private holds survive.
  Do not restore sequence-ranked order/deletion delivery, raw-ID comparisons,
  scalar visibility over removal receipts, or duplicate receipt writes.
- `81df09092` deletes the connected-provider refusal, blanket association hold
  and selector flag together. Selected Git/HTTPS round-trip and private-origin
  proofs pass; creation/link-origin and combined lifetime acceptance remain.
- Repeated JSON receipt decoding/creation validation inside dependency retries is
  removed. `ObjectChanges` prepares typed creation, deletion and order histories;
  the common receipt writers still own merges and conflicts. Snapshot validation
  rejects malformed input before retention. Scalar ranking excludes receipts and
  independent provider evidence; their separate protocols and origins survive.
- Common reconciliation's equal-desired exception after a changed provider baseline:
  adopt the observed value without acknowledging an unattempted local effect.
- Project operations' physical-ID-only scan: full IDs now use the common
  correspondence lookup, retaining repository scope and name ambiguity checks.
- Destination-only recovery presentation: CLI/Desktop now consume the same
  journal-derived per-origin values and membership, not a second recovery store.
  No effect writer, enrollment or candidate-as-confirmation path is introduced.
- Duplicate recovery owner resolution and status journal reads: import/recovery
  share `resolved_owners`; export status and recovery start from one validated
  journal. Only recovery expands private dependencies. Its failure cannot obscure
  readable export status; its values never become publication content.
- Capture/validation share `task_creation_input` and `project_creation_input`;
  exact captured inputs, Team/parent mapping and origin identity remain checked.
- #1499 integration removes follow-through's duplicate Task acquisition and its
  references to retired export columns. Common resolution and `planning_creations`
  own those reads. Remote source selection uses the Task's checkout branch, not a PR.
  Due dates join the same planning journal; completion requests remain local.
- Acquisition-side `inherit_project_placement`: provider readback is planning,
  not first local execution. Explicit placement retains its existing owner.
- State delivery's cache-only readback, which settled its local receipt without
  publishing the observation into the peer journal. Reuse common accepted-planning
  projection in the same transaction; retain the exact-current-receipt fence.
- Physical-only membership delivery lookup and raw-ID readback/baseline comparisons.
  Reuse explicit correspondence at delivery/comparison; keep captured IDs unchanged.
- Baseline/head-index compression: `610b13869:scratch/work-on-another-machine-name.md`,
  this heading. Preserve absent/null distinctions and separate creation origins.
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

Earlier head-value, creation-fixture, association-conflict and accepted-observation
reductions remain at `96f714bd6:scratch/work-on-another-machine-name.md`,
**Delete — do not maintain**. Preserve the single journal, per-origin heads,
exact common acquisition and accepted field frontiers; do not recreate the
removed writers or helper-only proofs.

Origin-keyed scalar winner grouping and the Linear-only foreign-predecessor
restriction are deleted. One resolved-owner
frontier traverses causal ancestors before ranking; the existing observation table
commits accepted source IDs. Per-origin journals/heads, losing edits, creation
origins and raw delivery inputs survive. Scalar relationship values resolve only
at projection. Associated order/deletion receipt projection now uses the common writers; the
private/projection holds remain independent of association.

Compression shares predecessor traversal between resolved-owner frontier selection
and delivery baselines. Walk shared ancestry once per owner, including paths through
unassociated origins without merging them. Selection checks reuse one object index,
not a rebuilt journal scan per association. Per-origin heads, private holds and
receipt composition boundaries are unchanged.

## Remaining integration — October 9

Receipt origins and full-ID correspondence exist. Ordinary import now projects
associated creation receipts onto their local owner without releasing private holds
(`39dba32c1`).
**Scalar and receipt composition have focused storage proof, including
state/comment/Wave readback (`8a6f8f807`).** Per-Work recovery presentation now
reads the retained journal through common status; item 3 still owns public
Git/HTTPS composition before review-only publication. Private or failed groups never advance
observations. The preceding state/comment/Wave direction is implemented, not a
second implementation step.

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

Integrated common comment acquisition/receipts (`7e56c01fd`), scoped status
(`de9470d5a`) and semantic content (`a518b985b`) retain their boundaries below.
Gate owns combined admission, cold-worker and provider recovery, including comments
acquired in a different order from Task creation; parent integration proves no
peer composition.

Creation/link discovery and receipts share `planning_export.rs` and
`planning_exports`; mappings and imported transitions never acknowledge effects.
Exact capture/readback proof: `95e81c688:scratch/work-on-another-machine-name.md`,
**Remaining integration**.

Connected-provider fixture history: `6f1869e9b:scratch/work-on-another-machine-name.md`,
**Remaining integration**. `a002e4060` passed public work-watch/edit/readback
against synthetic HTTPS; `098de8033`'s corrected `sync.changes` assertions await
gate. Its six-table comparison uses unrun peer-born records: no fabricated
placement/transition, not populated execution or running-control preservation.
Lost creation/link responses still require exact readback, not repeated effects;
this supplies neither mixed-provider exchange nor simultaneous creation authority.

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

- **Recovery presentation (`d6049af97`, `af7730926`).** CLI status and Desktop's
  Git planning disclosure expose per-origin membership, associated identity,
  references, assignees and lossless alternatives. Comment authorship is captured;
  scalar authors stay unknown. Candidates do not confirm projection or delivery.
  The common reader expands private dependencies only for inspection; export
  remains selection-scoped. Malformed private recovery history cannot hide readable
  export status or receipts. Ordinary edits own restores, with no new writer.
  The public setup fixture enters association/status after Store-seeded import,
  retaining private comments/parents, planning revisions, pending receipts and an
  empty remote ref. It does not exercise Git acquisition or populated live controls.
  Separate storage fixtures cover populated execution; headless Desktop checks
  assert displayed values. Combined transport and recovery UX judgment remain.
- **Accepted import observation (`dac02060a`–`96f714bd6`).**
  Ordinary same-origin projection records accepted scalar/content source IDs in
  `planning_peer_observed` within its savepoint, without echo. Local capture uses
  that frontier; rejected/held imports retain the prior observation. The fixture
  covers Task title and Project workflow through rejection, repair, local save and
  return import, with original baselines, repeated/older snapshots and populated
  execution. Focused observation execution is recorded below; released-frontier
  and revision checks remain with gate. The joint-owner cut reuses this mechanism, without
  proving public exchange or complete association recovery.
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
  and losing-edit recovery now use the common journal-derived recovery view.

`5bed3a211` integrated main `3a0aa5ca4` (#1510), after common writer `d20c56daf`;
`ce1750393` integrated #1512. These add chat recipes/hierarchical names and driver
recovery, not provider-frontier acceptance. Combined launch/Flow verification
remains with gate. The single remaining-work list below owns association,
public composition and recovery review; retained conflicts are not convergence.

Mixed-provider exchange uses the common selected-import and effect owners.
Private-selection holds remain intact. Combined verification belongs
to gate; publication still requires the acceptance below, with no landing authority.
Earlier cut-by-cut evidence: `3f7d9f2da:scratch/work-on-another-machine-name.md`,
**Remaining integration**.

Earlier acquisition-age/readiness and integration proofs: `610b13869:scratch/work-on-another-machine-name.md`, **Remaining integration**. Planning remains unplaced; observations, not errors, settle effects.

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
Connected Linear repositories use the same exchange owner. Public association
round-trip and private-origin proofs pass; broader invalidation and lifetimes remain.

1. Public `lf planning key/connect/use/select/status` now provisions/recover keys,
   pins an empty binding, selects exact Wave IDs and displays retained imports/holds.
   `use` routes only future root Waves; descendants inherit saved membership. Joining
   and switching never enroll existing work. Root routing and imported-root isolation
   live in the existing peer draft. Status omits credential-bearing endpoints.
   Focused setup and public creation/edit checks pass in isolated Linux.
   Setup never contacts the remote. Background exchange consumes the selection;
   full acceptance and presentation review remain.
   Holds and retained values have CLI/Desktop presentation. Selecting referenced Waves
   explicitly may release private history; returning to a selected parent cannot.
   Recovery UX judgment remains with review, not a new membership model.
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
   Desktop review judgment remain; per-Work selection, authorship and assignee
   presentation now exists in the common status view.
   No automatic turn/Flow retry exists.

3. Finish public creation/link-origin HTTPS recovery and combined negative-evidence/
   ordering composition. `81df09092` proves scalar association through ordinary
   Git exchange, HTTPS readback and subsequent local-save exchange, including
   reversed/repeated imports, populated execution and private-origin holds.
   Gate owns combined lifetimes; review owns recovery UX.

   Clearing a legacy mapping remains forbidden: it makes retained Work eligible
   for a new provider creation while old selectors and effects still belong to
   its original ID. Correspondence resolves full IDs without renumbering Work or
   moving execution. Mapping checks precede scalar and independent-evidence
   acquisition; losing mapping/creation claims retain effect holds. Saves and
   acquisition stay independent. Counterexample and earlier refusal-only proofs:
   `81df09092:scratch/work-on-another-machine-name.md`, item 3.

   **Receipt-origin cut — October 9, not a new product decision.**
   `planning_creations` separates each original creation/attachment identity from
   its local projection. Preserve captured UUIDs/models, per-origin uncertainty,
   projection-scoped locking and exact-origin readback/status. Nullable input keeps
   pre-capture discovery failures without inventing attempts. Released effects
   migrate directly into the final draft; obsolete per-kind columns/writers stay
   deleted. Detailed cut: `1cbbba625:scratch/work-on-another-machine-name.md`, item 3.

   Creation-origin composition and its exact-readback fixture are retained at
   `90a37ab79:scratch/work-on-another-machine-name.md`, item 3, **October 9 receipt
   composition**. Ordinary import retains every origin and captured input under
   its local owner, even when private selection holds scalar projection; no
   identity, selection or execution is reassigned. Focused storage proof covers
   reversed/repeated imports, uncertain origins, private membership and exact
   acquisition. This is not public Git/HTTPS acceptance.

   `planning associate <incoming-id> --with <local-id> --linear <provider-id>`
   now records a machine-local correspondence using the incoming journal's
   unambiguous scalar mapping and the existing local row's same exact mapping.
   It refuses redirecting an existing physical row. The common Task resolver and
   Project selector resolve associated full IDs only; direct identity readers
   remain exact. Membership, captured effects and execution foreign keys do not
   change. Lookup rechecks mapping/repository. Selected origins jointly project fields and receipts. Association alone retains
   its projection conflict; successful import releases it, not private or failed
   groups. Common receipt composition remains the only effect owner.

   The Store correspondence fixture imports older/newer snapshots in both orders,
   repeats association and resolves both IDs. It asserts unchanged private planning,
   local uncertain field receipts and populated execution; it does not project a
   cross-origin winner. The CLI fixture seeds/imports through Store APIs, then runs
   public association and Task status (not Git acquisition or provider readback).
   The recorded 11-test pass includes this public association case after
   `521c7040f` corrected its hold diagnostic; it is not public Git/HTTPS acceptance.

   Portable-observation rationale and the same-object counterexample remain at
   `01769ea0d:scratch/work-on-another-machine-name.md`, item 3, **October 9 portable
   observation cut**. Exact provider equality still governs unobserved sources;
   the joint cut below supersedes its Linear-only parent validation. Private
   dependency serialization proves no public exchange.

   Joint-scalar projection, accepted-source capture and portable-observation
   rationale: `f55d6c5b7:scratch/work-on-another-machine-name.md`, item 3,
   **October 9 joint scalar cut**. Per-origin journals, uncertainty, private groups
   and exact provider-equality rules remain; public composition is below.

   **Implemented association boundaries — reconciled October 9.**
   `79e478550`/`521c7040f` supply membership composition.
   `e422eca8b`/`19ed9a5a1` supply receipt-origin/order-member composition:
   journal-owned capture/ranking, cross-origin receipt-reuse rejection (including
   snapshot union), and alias-aware delivery/readback. Captured inputs survive;
   unknown correspondence defers readback rather than discarding a member.

   Ordinary-import fixtures cover partial-progress order readback, later-save
   rebasing and populated execution. Detail cannot settle order; equal issue
   revisions with different ranks require the complete provider body. Deletion
   retains origins and private uncertainty. Combined public ordering/removal
   acceptance remains distinct from the scalar Git/HTTPS proofs.

   **October 9 state/comment/Wave composition (`8a6f8f807`), implemented.**
   State/comment/Wave composition now has ordinary association/import fixtures,
   reversing/repeating snapshots and returning local saves after accepted peer
   observations. They preserve mutation IDs, authored losers, uncertain local
   attempts and populated execution. Association alone changes no Wave selection;
   later import resolves the selected Project through correspondence. Private
   comments, decisions and parent references remain held even after another
   destination is selected.

   The fixture exposed cache-only state readback: its receipt settled locally,
   but the peer remained pending because the journal lacked the observation.
   `observe_task_state` now enters common accepted-planning projection in that
   transaction, retaining its exact-current-receipt fence. No second writer or
   migration is added. Return readback settles the peer without replaying a turn.
   These are storage proofs, not public Git/HTTPS or running-control acceptance.

   **Public composition repair — October 9.**
   The connected-Linear refusal and blanket association hold are replaced together
   by the existing selected-import and receipt-conflict owner. Association alone
   still grants no effect; a successful import must project all selected origins.
   Private references and rejected projections retain their independent holds.
   The new public association regressions enter setup,
   association, work-watch Git exchange and synthetic HTTPS readback on two stores.
   The first composed run exposed a duplicate title write after a lost response:
   the receiving store's unattempted save remained pending when Linear already
   contained its desired value. Common reconciliation previously preserved that
   equal value, allowing another delivery before the next peer import.

   The repair adopts a changed Linear baseline even when it equals the desired
   value. It retains the receipt and both values without setting attempted or
   acknowledged; only matching actual attempts acknowledge. Unchanged baselines
   still preserve later saves. This is the existing Linear-wins policy applied to
   an observed fulfilled intention, not provider atomicity or a new retry owner.
   The membership fixture's earlier pending-unattempted assertion is superseded:
   desired readback retires delivery without inventing the local effect. The
   prior assertion remains at `f55d6c5b7:rust/loopflow/src/store/sqlite/planning_peers.rs`,
   the associated-membership fixture; this section records the public counterexample.

   Public Project workflow lookup exposed another bypass: operations scanned
   physical IDs instead of the common correspondence reader. Full-ID operations
   now use that reader, with repository scope retained.

   The public regression checks both full IDs, Task/Project causal baselines,
   uncertain losing receipts, populated execution, repeated association, older and
   repeated Git documents, and private associated origins. Exact
   creation/link-origin HTTPS recovery and combined negative-evidence/ordering
   composition still need their public proof; existing storage fixtures are not
   that proof. Gate owns combined command/lifetime verification afterward; Jack
   Heart's review-only boundary is unchanged.

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
   reads. Public invalidation and complete receipt composition remain gate coverage
   before publication. This
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
   The same harness now covers two-store/disposable-ref Git associations; it does
   not replace native peer-lifetime verification.
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

`2df907de7`/#1516 and `6fdd7f09e`/#1511 integration details remain at
`610b13869:scratch/work-on-another-machine-name.md`, this heading. Native peer
acceptance still needs supported Codex 0.161.0, not the Claude stand-in.

`610b13869` integrates #1499: checkout branches no longer require a PR;
completion requests remain local, independent of planning disposition. Retained
sync repairs update receipt queries to `planning_creations` and historical
follow-through capture. The combined cut removes duplicate acquisition,
preserves exact old creation inputs, and carries due dates through the existing peer
journal. Counterexamples: a placed unpublished Task has no active PR; a captured
creation can omit optional null fields; a peer-born follow-up otherwise loses its
due date. No follow-through event, completion request or PR authority is exported.
Six focused checks cover due-date acquisition/removal, preserved execution and
completion requests, captured due dates, old receipt migration, historical filing,
lost creation/relation readback, and unpublished source-code refusal. This is not
public two-store creation/link-origin or running-control acceptance.

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

Earlier scalar Git/HTTPS, state/comment/Wave, compression and 77-test evidence:
`610b13869:scratch/work-on-another-machine-name.md`, **Acceptance for review**.
Those results predate #1499 integration and prove no combined native launch path.

Check: fmt, all-target Clippy, lib/remote test builds and diff check pass; `uv run python scripts/test_network.py <lib-test> peer_due_date_readback peer_creation_prepares_unprepared peer_migration_preserves historical_filing_converts follow_up_export_recovers --test-threads=1` passes (5), as does `<remote-test> machine_selector_names_unpushed_source_work_before_connecting_and_keeps_legacy_identity --exact` (1). Gate owns materialized/public composition and lifetimes; review owns recovery UX.

SQL/macOS and unresolved Flow-exit evidence: `eeb98aa89`, this file's
**Acceptance for review**. No provider death, replacement authority or installed
acceptance follows. Parent-sync timeout: `a6cb48664`, same file, supplies no test
verdict. LOO-406's landing authority excludes this review-only Task.
