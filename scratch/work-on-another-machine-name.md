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
- Raw Project Markdown mutations, duplicate workflow representation and a SQL
  content parser. `project_content.rs` owns semantic workflow/KR/target capture,
  persistence and populated migration; existing list receipt granularity stays.
- Peer-only scalar/state/comment receipt writers, reconstructed comment provenance,
  identical-body acquisition bypasses and replay of superseded provider revisions.
  Common acquisition owns validation; savepoints isolate valid contradictions and
  retain membership evidence after rollback. Entity revisions never order relationships.
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

Test-only `export`/`import` helpers remove repeated successful-call plumbing, not
assertions or failure-path checks. No predecessor path or second planner is added.
Remaining grouped owners, legacy association and acceptance are below; omission
and retained conflicts are not convergence.

## Remaining integration — October 9

Current checkpoint: `ffbe0ad42`, integrating parent `cbf0a174a` (LOO-406).
The conflict-free sync adds terminal-Task guidance ordering, prompt-fixture repository
isolation and testing guidance; it does not complete peer composition. Prior focused
proof at `19f0a4b6d` remains evidence for its source, not acceptance of this combined
candidate. Task admission and cold-worker behavior remain in gate's affected checks.
Destination-scoped status/conflicts (`de9470d5a`), common comment acquisition and
receipts (`7e56c01fd`), and semantic Project content (`a518b985b`) are integrated.
Their owners and preservation boundaries are described under **Delete — do not
maintain** and item 4 below.

The next implementation boundary is item 4's creation/link attempt transport and
Project readback through `planning_export.rs`, followed by ordering/deletion receipts
and alternate provider/relationship acquisition. A peer mapping or retained import
checkpoint cannot settle an uncertain provider effect. Existing Task readback only
reconciles a receipt already present locally; it does not transport that receipt.
Legacy association and Desktop presentation then complete the remaining surface.
Only after those owners compose can mixed-provider exchange be enabled and its
public reconnect/preservation acceptance run. These are implementation gaps, not a
missing parent dependency or verification-only work. Publication still requires the
acceptance below; landing remains excluded. Earlier cut-by-cut evidence remains at
`3bebc219d:scratch/work-on-another-machine-name.md`, this heading; the focused
executable results below supersede its blanket Rust-unavailability claims.

Executable feedback took priority over creation/link composition on October 8.
The isolated Linux build exposed invalid registry-error conversion and SQLite
integer/ID bindings. The focused comment test then found imported Tasks missing
required local `workspace_slug` and `updated_at` initialization. New peer Tasks
now use the common writer's empty placement and local timestamps; existing
execution remains untouched. Fixtures retain those same valid local defaults.
Review kept the strict reader and portable-field exclusion rather than adding
fallback values or exporting placement. Prior global Rust-unavailability claims
are superseded by the focused results below, not by full composed acceptance.

October 9 implements Task creation **readback**, not Project/link or attempt transport.
Accepted peer provider bodies now reconcile the original `planning_export.rs` receipt even when
an earlier mapping-only import already attached the identity. Mapping alone retains
uncertainty; the captured save boundary limits acknowledgement. Later losing saves,
creation attempts and their identities survive. Review rejected an added check
freezing Project membership to its creation snapshot: a later accepted move must
remain possible. Project/link readback retains its existing owner until full receipt
composition supplies explicit settlement. The focused regression covers mapping-only
import, lost creation reply, later save, selected Linear winner and idempotent readback.

Executable foreground feedback exposed missing acquisition ages and a readiness
check that equated absent list inventory with invalid membership. Import retains
the accepted provider age, without synthesizing membership-list evidence. Positive
archive, membership and configured-Initiative contradictions still block readiness.
Imported planning remains unplaced; a later common placement operation selects the
local machine only when neither this Work nor its parent has a placement. Existing
placements survive. The public work-watch regression now covers cold creation on
both stores, offline comments/edits/completion, reconnect and retained execution
including Machine placement. It uses synthetic provider facts without a connected
Linear service; it does not establish mixed-provider delivery or real SSH.

The earlier October 8 sync integrated parent `ffe986160`, including
`078a6642e`'s gate fixture/DTO repairs and reported Linux migration, native/public
Flow/work-watch reconnect and adoption passes; Swift remains unavailable. Those
results concern LOO-406, not this combined peer candidate. `ffe986160` removes
synthetic state settlement: provider observations settle receipts; failures only
record errors. The peer lost-reply fixture now uses `task_state_error`, retaining
its uncertainty assertions and this branch's `record_in`/`adopt_peer_in` ownership.
No compatibility shim or duplicate settlement writer is needed.

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
Linear-connected repositories still do not exchange: grouped receipt and provider-path
composition remain unfinished. The safety boundary's diagnostic still names the
provenance gap; revise it with the completed composition, not as a product limitation.

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

   October 8 source cut replaces the boolean Linear-origin flag with a retained
   observation (provider body/identity/revision and original acquisition time).
   Common acquisition's accepted facts feed one field map; only matching projected
   values get provider provenance. Pending local values are not relabeled. Equal-value
   observations, including first acquisition of migration baselines, enter the journal
   when no head carries that provider frontier. Concurrent Linear heads order by
   provider revision before local clock; causal successors remain successors.
   Peer projection reuses `put_item`/`put_project` and membership validation inside
   its savepoint, advancing existing provider tables rather than adding a frontier
   owner. Repeated acquisition emits no mutation. Payload validation rejects extra
   execution fields and a field disagreeing with its claimed observation.

   Authored Task coverage imports a newer/equal-value frontier, then reads older,
   equal-value and contradictory same-revision provider responses, retaining an
   uncertain title receipt, losing value, Session and checkout without echo. Project
   coverage imports an equal-value frontier and rejects an older detail response;
   it does not yet cover the Task test's full receipt/contradiction matrix. A separate
   Task test preserves a pending local value on an unchanged provider baseline.
   The focused storage suite passes; connected-provider composition remains unproved.

   Creation, ordering and deletion receipts remain uncomposed. Comments now use the common owners described below. Task disposition now uses the common receipt owner;
   authored regressions cover lost replies, later reopening, late readback exclusion,
   unchanged-state provider conflict, idempotence and transaction-wide receipt failure.
   Cancellation passes in that storage test. Common accepted Task/Project acquisition captures
   observations; other provider writers still need coverage. Comment payloads now retain the raw provider observation and use common
   acquisition/delivery; public mixed-provider lifetime acceptance remains. Project
   relationships and list ordering have independent frontiers; entity updatedAt
   cannot order them. Raw observations are immutable evidence in field receipts,
   not permission to invoke execution or acknowledge complete-list acquisition.
   October 8 source repair moves common acquisition before field/delivery projection.
   `put_project` now returns acceptance, like `put_item`; a rejected final frontier
   rolls back that object's savepoint while its journal and conflict remain.
   Removal/archive checks also apply to local winners and identical cached bodies;
   the old equality shortcut is deleted. Common cache upserts omit unchanged writes.
   Only the newest entity-revision group is acquired; equal-revision observations
   still undergo common conflict checks. Older unversioned history stays in the
   journal rather than being replayed against a newer accepted frontier.

   Membership rejection is typed. After rolling back the object, import reasserts
   the existing `membership_unresolved` marker outside its savepoint, in the import
   transaction; no second evidence store is added. Projected relationship fields
   must also match accepted membership even when authored without provider provenance.
   A preexisting unresolved marker cannot be cleared by entity acquisition. Neither
   a newer entity revision nor a peer clock orders Initiative/Team relationships.

   Authored regressions cover local/provider winners against retained removal and
   archive, relationship-only and newer-entity membership conflicts, independent
   Wave import, full journal retention, unchanged Task/Project/Session/Process/
   Workflow/checkout state and idempotent readback. A repeated newer-Project import
   covers retained unversioned history. These storage regressions pass; no composed
   CLI/Desktop acceptance is established.
   October 8 implementation replaces those acquisition owners' ordinary
   `InvalidData` for valid but unordered/contradictory observations with typed
   `ProviderObservationConflict`. Peer import retains it through the existing
   per-object savepoint/conflict path; malformed input and unexpected SQL still
   abort the complete import. No second frontier or exception-string matching is
   added. Direct provider acquisition still rejects the contradictory observation.
   New Task/Project peer-import regressions construct independently accepted bodies
   at the same revision. They require both journals, accepted body/age, uncertain
   receipt identity/value/baseline/attempt/error, Task/Project/Workflow and execution
   to survive while an independent Wave and checkpoint commit. Repeated import
   must be idempotent. Malformed revision, extra execution payload and mismatched
   observation/value fixtures require whole-import rollback. These storage tests
   pass; they do not establish composed behavior.

   Complete grouped receipts through their existing common owners, not the scalar
   field loop. Source inspection identifies the remaining seams:
   - `planning_export.rs` owns creation/link attempts. Accepted peer Task observations
     reconcile original receipts after a mapping-only import, without acknowledging
     later saves. Portable creation/link attempt capture and recovery remain.
   - Peer comments now reuse `task_comments.rs::ingest_task_comment` and
     `insert_authored_comment`. Comment ID remains delivery identity; reimport
     retains errors/acknowledgement/conflicts, and acquired comments create no
     delivery. The common reader adopts provider edits, retains losing receipts,
     rejects same-revision contradictions and returns stale rejection to both peer
     projection and foreground direction ingestion. Peer import never appends steers.
     Raw `IssueComment` replaces reconstructed display-only provenance; the shared
     conversion retains body, displayed author and creation time. Equal-value
     revision acquisition captures missing facts, while readback emits no new mutation.
     Migration omits acquired comments from the journal until a real acquisition
     supplies their provenance; authored pending comments retain identity. Omitting
     only content was rejected during review: incomplete objects abort import.
     The frontier test covers later capture of that retained row. The SQL proof
     also caught JSON key ordering preventing content provenance; trigger groups now
     match the field map's canonical order. New Rust regressions cover authored and
     provider-only comments, lost replies, later edits, retained losers, isolated
     contradictions, malformed input and unchanged execution; the storage suite passes.
     Public reconnect and foreground direction regressions remain gate acceptance.
   - `task_state_delivery.rs::queue_in` now shares `record_in` with peer projection.
     Peer receipts retain the mutation ID, causal baseline and projected completion
     time; `move_seq` stays null. Reimport retains attempts/errors; a later save
     supersedes without deleting uncertainty. Common reconciliation distinguishes
     acquisition from a selected concurrent Linear winner: the latter retires a
     losing intention even at its unchanged baseline. Neither moves a Workflow.
     Non-lifecycle cached states (for example started/backlog) project without
     inventing a local state intention. Imported completion-time preservation passes;
     alternate-provider-path coverage remains.
   - Project content now travels as semantic workflow/KR/target fields, not raw
     `project_prompt_context`. `project_content.rs` owns parsing and combined
     persistence; peer receipt projection uses its common field-delivery owner.
     Each KR list and target list is one existing semantic field, not a new
     per-item identity model. Acquisition records original provider provenance
     only on equal values. Populated migration and runtime writers use the same
     capture owner; import suppresses echo. Storage tests pass; composed acceptance remains.
   - `planning_order.rs::observe_in` requires complete-list evidence; Task entity
     revisions and scalar peer fields cannot settle a captured move. Deletion
     retains its separate baselines and attempts in `planning_changes.rs`.

   Cover alternate accepted provider writers, including reteam, archive/removal,
   complete-list order and public comment acquisition, not only Task/Project detail reads.
   Existing local writers are evidence for reuse, not proof of peer composition.
   Then finish legacy association and Desktop
   status before enabling mixed-provider foreground exchange and replacing its
   temporary diagnostic. This cut cannot repair LOO-406's unseen provider-write
   race or supply Linear compare-and-swap semantics.

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

Jack Heart requested stacking on LOO-406 and continuing pursue. Current stack
`ffbe0ad42` integrates parent `cbf0a174a`; earlier `a6cb48664` integrated
`7c626ca23`, preserving restored scratch checkpoint `1ad5e1267` and the retained
stash. The October 9 sync changes no publication or landing authority. Earlier
parent PR #1503's published base `b6f34a6f8b811d95f9fc843ac6592cc2262f2e26` is integrated,
including `e68f2a423`'s creation receipts, `b1f175bbf`'s ordering,
`6bb8b935a`/`086d3560c`'s common sync view and `29777b8cb`'s repository-scoped
foreground lifetime. Reuse those owners; the earlier `e68f2a423` integration
reference described the first writer cut, not the current stack frontier.
Its common ownership cut `84664e661` removed `PlanningAuthority`, personal-plan storage and
split writers. Source: `e68f2a423:scratch/explore-loopflow-s-own-store.md`.
The missing-writer diagnosis at `4f9a8ea17` is superseded. LOO-412 owns remaining
peer integration independently of unfinished Linear delivery; no second planner.

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
Its GOAL.md and full MEMORY.md were reread on October 9; filesystem inspection
found no other immediate Infrastructure child memory, including unregistered scopes.
Those operation-entry and result-accounting lessons are already in parent memory.

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

The compression check exposed an inherited storage fixture that expected
`select_project_workflow` to parse malformed YAML. `ops::project::workflow` owns
that parse before saving; storage owns name validation and atomic persistence.
The fixture now rejects an invalid Workflow name and verifies the stored definition
and selection survive. No duplicate YAML parser was added. Earlier passing
comment/import/setup proofs remain at
`3bebc219d:scratch/work-on-another-machine-name.md`, this heading.

Compression review repaired fixture setup, not production guards: Session insertion
already records Started, so fixtures no longer overwrite it; absence uses the optional
Task reader, and seeded Projects retain their required timestamp. Shared execution
setup keeps the Session, Process, Workflow and checkout assertions intact. Prior
focused Project-content and public work-watch evidence remains at
`b0f2a00702ccdbe1b11c22fa55f472cc649bf8d1:scratch/work-on-another-machine-name.md`,
this heading. Public status/DTO, cold-worker and provider-lifetime execution remain
with gate, alongside Desktop and mixed-provider acceptance.

Recorded proof at `19f0a4b6d`: isolated/network-denied Linux peer-storage suite passed
all 46 tests; all-target Clippy and fmt passed. The sync did not change that storage
module. Public Task admission changed upstream, so these results do not establish
combined cold-worker, provider-lifetime or mixed-provider acceptance.

Check (October 9 realign): `git diff --check` and `lf context --skill realign` pass; prose-only reconciliation reuses `19f0a4b6d`'s focused proof; gate owns the combined candidate's remaining acceptance.

Prior SQL proofs and macOS startup samples remain at
`e77d2d1c8:scratch/work-on-another-machine-name.md`. A fresh bounded attempt
again stopped at `_dyld_start` before the build script, with a valid signature;
this is not a diagnosed cause or a Rust failure. Disposable Linux compilation
bypassed that environment boundary and exposed the defects repaired above.
No installed binary, security setting, credential or host store changed.
Historical Flow `93d4d4f6-4723-4027-951b-b3aee2e696a2` still retains realign
step `e94edd38-2a26-4047-9650-41a65ab814f7` without a recorded exit, confirmed
by `lf flow show --processes` on October 9. No exit or replacement authority is
inferred. Its reconciliation checkpoint `e8ff4c7f6` remains preserved.

Parent-sync timeout evidence remains at `a6cb48664:scratch/work-on-another-machine-name.md`; it supplies no test verdict. LOO-406's landing authorization does not extend LOO-412's review-only boundary.
