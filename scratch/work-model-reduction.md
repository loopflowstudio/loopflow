# One Work model — accepted deletion audit

2026-10-10 · LOO-412 · Jack Heart accepted the structural redesign in
[work-sync-redesign.html](work-sync-redesign.html) and requested this deletion audit.
The exact `WorkEdit` shape belongs to implementation; it requires no further design
approval from Jack. This is accepted scope, with conditional deletions identified below.

## Scope and outcome

This audit and [the pursuit design](work-on-another-machine-name.md) are one
accepted delivery scope. Canonical readers and narrow acquisition evidence are implemented; provider bodies now have concrete observation columns; adapter confinement remains,
followed by typed edits/journal, ordinary follow-ups and one cron list. Concrete
bindings are implemented (§14). All precede Jack Heart's next
review of PR #1491. Publication is review-only; landing, installation and export
of actual Work to the public repository remain unauthorized.

Stopped-Flow recovery evidence: `09709a05cc:scratch/work-model-reduction.md`.
Native completion and the recorded iterate decision do not establish an lf exit
or authorize replay. Older provider uncertainty remains intact.

The result should be one ordinary local reader and typed writer for Wave, Project,
Task and subordinate Task comments. Linear and Git translate changes into those
objects. They retain the evidence needed for synchronization without owning another
writable Work hierarchy. Execution state is machine-local.

## Audit basis

Definitions, consumers, SQL, configuration, transports, CLI/DTOs, Swift, fixtures,
skills and docs were inspected. Earlier audit tracing remains at
`f9f9da7d3a:scratch/work-model-reduction.md`; paths below use `rust/loopflow/src/`.
Remove obsolete responsibilities with their consumers, retaining required invariants.
Whole-file and whole-table deletions remain conditional where stated.

## 1. Domain wrappers and the missing fields

**Removed:** ordinary provider-shaped Work readers and `PmProject`.
`PmItem` and `LinearProject` still escape the adapter; public snapshot/detail
envelopes and their generic-provider ingestion APIs are removed.
`09709a05cc` deletes `PmWave`; setup consumes the existing private Initiative response.
Issue observations no longer duplicate the Project display slug; list acquisition
requires only its ID. `ProjectRef` and fixture display reconstruction are deleted.
ProjectPlan, TaskPlan, live Project iteration and public ProjectContent are removed.
ProjectKR, MetricTarget and domain TaskComment are implemented.

**Locations:** `work/project.rs`, `work/task/mod.rs`, `work/task/comment.rs`,
`pm/mod.rs` and their SQL serialization in `store/sqlite/`. The root
`planning.rs` module is deleted; §11 records its surviving owners.

Project persists description, workflow, KRs and targets directly; Task persists
rank, assignee, due date, state and completion. ProjectStatus belongs to the domain.
Keep optimistic revisions, timestamps, historical iteration events and optional
Linear mappings. Whole Task records still cannot travel because they carry local data.

Project ingestion/association return no provider record; `plan_read` and the public
`pm_snapshot` inventory reader are deleted. Production snapshot builders are removed.
Confine `PmItem` and `LinearProject` and the concrete acceptance APIs before
considering this cut complete. Composed Project prose saves,
delivery and readback preserve field revision/age across partial observations;
that proof does not satisfy the structural cut. Earlier tracing:
`f9f9da7d3a:scratch/work-model-reduction.md` and `582c134439` at this path.

## 2. Delete the second local reading stack

**Removed:** `ops/project.rs::ProjectPlanning`, public `planning_project(s)` reads,
and the Wave/roadmap Project inventory join. Selection, edit/workflow, rotation and
CLI/Desktop Project summaries use canonical Work and explicit Linear bindings.
The canonical list preserves shared rank rather than edit-time order. Project and Task
export read canonical Work. Rotation also consumes/returns canonical Tasks; it no
longer substitutes IDs or re-resolves a fabricated PM item.

**Removed:** `planning_task` and `plan_read::{task_in,project_in}`, including fixture
dependencies. Task delivery reads accepted revision evidence directly from Work,
not observed provider revision. Canonical status and summaries retain separate
acquisition state/age. No local/provider-ID substitution remains in that reader.

**Removed:** `sqlite/children.rs` status/admission reconstruction through
`pm_task_observation_in`. One narrow SQL read uses retained membership, Team,
invalidation, removal and age evidence; pending local membership affects display,
never admission. Acquisition now reads narrow Linear identity, revision, ownership,
validity and age too. `PmTaskObservation`, `pm_task_observation_in` and
`read_task_planning_async` are deleted. Cached acquisition returns canonical Work;
delayed absence retains its revision/age fence and Wave-lock lifetime. Raw provider
input replay lives only in fixtures. Adapter confinement remains.

**Keep:** explicit missing, invalid, removed and unavailable evidence. A failed
provider read must not erase cached Work or convert unknown state into absence.
Return canonical IDs with optional bindings and acquisition information; do not
create another generic reader framework simply to replace these functions.

## 3. Collapse accepted-provider-to-Work projection

**Implemented persistence cut:** `planning_peers.sql` replaces `pm_projects.body`
and `pm_items.body` with concrete observation columns. `put_project`/`put_item`
compare fields; projection, receipt baselines, ordering, acquisition, admission and
selection read those columns. Prose keeps independent revision/age. Issue membership
and date presence distinguish omitted facts from observed null. Historical cutover
and uncertain-effect receipts retain their original bytes; new disposition conflicts
capture state evidence rather than a reconstructed issue.

**Removed:** `PmSnapshot`, `PmSnapshotRow`, `PmTaskRecord`,
`Store::put_pm_snapshot`/`put_pm_task` and both production snapshot builders.
Inventory/detail acceptance takes concrete Linear fields; all acceptance, reteam,
archival and ordering paths now remove the provider-selector argument. Projection
consumes IDs and batches retained-row receipt attachment, without fetched models.
Captured inputs stay in `tests/support/linear_input.rs`; transactions and the
blocking writer's cancellation-safe Wave-lock lifetime remain unchanged.

**Delete — do not maintain:** escaping `PmItem`/`LinearProject`, public concrete
acceptance methods and the remaining generic PM/provider hierarchy. Adapter
confinement is unfinished. Retain foreign/unmapped facts, contradictory ownership,
partial inventories, independent membership/removal and original ages. Whole-table
removal remains conditional on another existing owner retaining every fact;
canonical Work must not absorb foreign input.
`pm_wave_sync`, `pm_wave_projects`, `pm_issue_changes`, relation observations and
`pm_project_name_cutover` retain independent evidence, not ordinary Work content.

`require_accepted_initiative`, membership validation, revision ordering and
`reconcile_linear_project_teams` remain acquisition responsibilities. Explicit reteam
changes Team relationships without replacing independently newer entity facts.
Repository alias repair and revision triggers still operate on the surviving
observation rows. No ordinary reader reconstructs the removed JSON body.

## 4. Preserve secondary-reader evidence

`PlanningChanges::observation` is deleted; receipts read their selected column and
revision. Prose keeps its independent revision, deletion captures `deleted=false`,
and membership normalizes to local identity. Unrelated malformed ordering evidence
cannot prevent a name receipt. Same-revision archival/disputed membership still
refuses delivery. Populated released-0.13.11 conversion and rollback fixtures cover
the current columns; the pursuit design owns the current check result.

## 5. Delete the internal Markdown storage round trip

**Locations:** `sqlite/project_content.rs::{update_project_content,select_project_workflow}`, `sqlite/planning_write.rs`'s
special Project reads/writes and `ops/chapter.rs`. The former `pm/mod.rs` Markdown
functions now live privately in `pm/project_document.rs` for Linear and migration.

Structured persistence and removal of `read_content`/writer Markdown assembly
are implemented. Ordinary operations now take typed fields directly; Chapter input
is flat. The redundant `save_content` forwarding layer and `write_content` value mirrors
are deleted. Project saves and rotation derive receipts from the typed edits,
sharing acquisition’s Linear field mapping; rotation no longer reads PM projections. Migration parses
released Markdown before journal seeding; malformed content refuses without losing original bytes. Conversion and
Linear edits share recognized field ranges, preserving fenced examples and unknown
prose. The private `pm/project_document.rs` parser/renderer owns managed description
sections through one `ProjectDocument`; the nested `LinearProjectFields` and
forwarding parser are deleted. Prompt assembly renders its own output. Preserve unrelated authored prose
and partial-response omissions. Wave workflow definitions and a Task's captured
Workflow are different responsibilities and remain separate.

## 6. Replace duplicate edit vocabularies

**Locations:** `sqlite/planning_write.rs`, `planning_exchange.rs`,
`sqlite/task_content.rs`, `sqlite/project_content.rs`, `ops/task.rs::TaskEdit`,
`ops/planning_delivery.rs::task_input`, `pm/linear.rs::project_field_input`.

Current `PlanningEdit` generates kind/field/value/from_value/fields while callers
also pass `PlanningObject {kind,id}`. `PlanningMutation` repeats the field address
and untyped JSON payload. The writer validates combinations the representation
itself could exclude. Use one typed Work edit payload with its target; derive
transport conflict addresses/encoding from it. Delete the second kind/field schema,
repeated string-field switches. Team/Initiative edits now carry `Vec<String>`;
comment edits carry `TaskCommentAuthor`. Persistence and Git use native JSON values.
The unshipped string-format decoder and its compatibility-only fixture are removed: Jack Heart excluded old-version
support. Released SQLite conversion still preserves relationships, authors and
uncertain receipts; selected-format history retains mutation IDs, clocks, parents
and competing values. `ensure_record` consumes
typed edits directly instead of reconstructing a field-addressed JSON object.
The target-bearing enum/journal redesign remains required.

Task/Project saves and Task refiling now derive Linear receipts from the same typed
edit slice as the writer; Task saves no longer read `plan_read::task_in`. The
Project-only save forwarding and parallel Task JSON receipt values are deleted.
The journal/address conversion described above remains required.

Keep atomic disposition updates, comment identity, ownership validation, and the
transaction boundary covering local changes/imports. Serialization at the actual
Git or Linear boundary is necessary. A comment stays subordinate to a Task, not a
fourth Work scope. Existing `WorkRef` still names Wave/Project/Task.

## 7. Delete whole-model JSON patch reconciliation

**Locations:** `sqlite/planning_changes.rs::PlanningChanges::{record,normalize,
reconcile}`, `planning.rs::PlanningChange`, field-producing operations and the
Linear delivery adapter.

Linear content-field replacement is typed and preserves recognized Markdown
ranges. Whole-model `reconcile<T>` is deleted: Project and Task acquisition now
reconcile the typed edits passed to the common writer. An explicit Linear field
mapping limits receipt settlement to supplied fields; provider models are no longer
serialized, patched and reconstructed. Task membership uses canonical Project IDs.
Composed prose proof is now recorded above; the private-observation boundary and
provider-cache reduction remain incomplete.

Remaining: remove parallel edit structures that only rename the same values
(`name`, `issue_title`, `TaskTitle`, for example), receipt string-field addressing
and duplicated normalization of provider IDs into local owners. The typed journal
conversion still owns those cuts.

Keep attempted values and their baselines: an acknowledgment of an older attempt
cannot clear a newer local edit. Preserve conflict values, rebasing of edits not
yet sent, and exact evidence for an uncertain write. Removing a generic patcher
is not permission to replace these rules with last-arrival-wins.

## 8. Simplify Git journal, import and enrollment

**Locations:** `planning_exchange.rs`, `sqlite/planning_peers.rs`,
`store/planning_peers.rs`, `planning_git.rs`.

Delete `PlanningKind` / `PlanningObject` as a second public Work vocabulary and
redundant mutation validation once typed edits define the wire payload. Simplify
`import_peer_planning`'s generic object reconstruction through the common writer;
remove transport-specific creation paths and repeated kind-based ownership SQL.
Keep parent dependency handling, atomic application and failed projections.

**Conditional:** `enroll_repository` scans Wave/Project/Task/comment tables on
use/import/export to populate `planning_members`; `planning_active` mirrors selected
routing. Prefer current ownership plus configured transport and routing evidence on
the journal. Delete these extra inventories only after proving destination history
survives ownership moves, config switches, rejected records and tombstones with no
live Work row. Current membership alone cannot recover historical routing.

Keep causal heads, competing values, applied-versus-observed distinction, exact
remote/ref identity, publication readback, cancellation-safe locks and bounded
network collection. Preserve retained Git documents/history without inventing
cross-version negotiation or silently selecting a fresh ref.

## 9. Git recovery reads the journal directly

Removed `PeerPlanningRecord` / `PeerPlanningValue`, their reconstructed inventory,
constant-empty metadata and exclusive fictional fixture cases. Status returns the
retained `PlanningMutation` map plus ranked candidate IDs; CLI/Desktop show these
same entries. Values, causal parents, clocks and projection failures survive.
Candidate means ranked, never applied or published. This is inspection, not another
writer. The typed edit/journal conversion in sections 6–8 remains outstanding.

Journal read failure already returns `StoreResult::Err`; WorkReading retains cached
status and shows that failure. `pending_local` is therefore a required boolean on
successful reads, not an invented partial-read state. Publication and acquisition
errors keep their distinct owners. No schema or historical evidence is removed.

## 10. Unify selectors and acquisition orchestration

**Locations:** `ops/pm.rs::{PmContext,RepositoryPmContext,resolve_saved_task,
inspect_task_planning_async,acquire_task,resolve_owned_issue,
list_pm_waves,list_local_waves}`,
`ops/planning_sync.rs::find_task`, `ops/run.rs`, `ops/task/remote.rs`.

Task status, planning lookup and repository routing now use the canonical scoped
selector, including repository aliases; explicit Task IDs retain their identity.
`dfa2cdafba` makes `acquire_task` return canonical Work after narrow retained
evidence checks. `inspect_task_planning_async` remains the bounded refresh owner;
its name is not evidence of provider reconstruction. Remaining: overlapping
transport/context orchestration and escaping ingestion shapes. Retain refresh/lock behavior.
The unused `pm_show` inventory/DTO and `load_show_snapshot`/`read_pm_snapshot`
are deleted. Status sync refreshes before its canonical read; acquisition returns
no inventory. The unused `SnapshotPlan` hard-refresh distinction and exclusive band test
are deleted; actual acquisition failure retains its error.
`PmPortfolioValidator` and ordinary portfolio reconstruction are deleted. The
store checks narrow retained ownership claims directly; no inventory leaves it.
`e3a3479df7` removed the public `pm_snapshot` reader and all callers. Fixtures
read canonical Work or concrete retained evidence and replay fetched inputs;
public concrete acquisition inputs and provider hierarchy remain required cuts.

`PmResolvedTask`, both `pm_resolve_task` wrappers and `ResolvedTask` are deleted;
`ops/task_pm.rs` is removed. Acquisition validates concrete Linear ownership and
returns the saved Task. The acquisition/lookup split and provider-shaped ingestion
inputs remain.

Linear setup, OAuth, Team/Initiative mapping and reteam are real integration
operations. Keep them concrete. A local lookup cannot infer that another Machine
has never started work; this rewrite grants no distributed execution authority.
Remote admission must still fetch pushed code and retain dirty or divergent targets.
The planning-only `require_planning_home` restriction is deleted from Task refiling
and chapter rotation; execution admission and sorted planning locks remain.

## 11. Keep necessary delivery evidence without a generic hierarchy

**Locations:** `store/planning_sync.rs`, `ops/planning_delivery.rs`, `ops/planning_export.rs`,
`sqlite/planning_sync.rs`, `sqlite/planning_export.rs`, `sqlite/planning_order.rs`,
`ops/linear_observe.rs::PlanningSync`.

Root `planning.rs` is removed: `NewTask` belongs to `work/task`, Linear IDs and
`LinearBindingError` to `pm`, and receipt/status types to `store/planning_sync`.
No compatibility reexports remain. Remove redundant content copies used for display and
duplicate field translation. Keep immutable creation inputs where an uncertain
external effect really needs them. Move the foreground exchange coordinator to
its actual owner; its current Linear-named module also handles Git.

Do not unify creation, comments, ordering, deletion and disposition into a new
universal effect lifecycle merely for symmetry. Their existing receipts,
acknowledgments and readback rules have distinct jobs. Offline buffering remains
local; there is one configured transport and no fallback transport or resident.

## 12. Remove dual-identity joins from CLI and Desktop

**Rust:** ProjectSummary now reads canonical Projects once, has no optional
`work_id`, and requires sync evidence. CLI and Swift select local identity with
an explicit optional `linear_id` binding. Project operations and rotation return
canonical Work. Wave/roadmap/Work-frame Task summaries now read canonical Tasks, with local IDs,
explicit optional Linear bindings and required sync evidence. `PmTaskSummary`,
`planning_wave` and their dual-ID joins are removed. Creation now returns canonical Task and the CLI uses its title; `task_planning_item`
is deleted. Placement/PR consumers read explicit branch/URL metadata. Task status now returns canonical Task/Project records; summaries expose acquisition
state/age independently of delivery, including Swift decoding and display. Remaining:
escaping concrete Linear inputs. Project delivery now checks accepted Work revision
and retained archive/membership evidence, not a returned provider record.

**Swift (under `swift/`):** `Models/WaveWorkMap.swift`'s Project/Task planning
snapshots and `PlanningKeyResult`; `Models/WavePlan.swift`, `PlanningSync.swift`,
`PeerPlanning.swift`; `Services/RegistryQuery.swift`, `WorkObservation.swift`;
Mac WorkModel and PeerPlanningView/PlanningSyncView/ProjectHistoryView/RoadmapView/
WaveDetailPane/WorkSurfaceView/TaskWorkView consumers. Update corresponding wire
fixtures together, with no defaults masking drift.

Keep useful compact DTOs and UI state. A Task runtime summary is not a competing
Task owner merely because it has a different shape. Preserve selection, cached
content, independent Session reads and independently failed frame parts. Shared
completion must update displayed Work without moving a local Workflow or cleaning
up its checkout.

## 13. Finish every follow-through producer and consumer

The existing pass owns this cut; the deeper pass must not leave indirect producers:

- `work/task/follow_through.rs`, `ops/task/follow_through.rs`,
  `sqlite/task_follow_through.rs`, special CompletionGate/settlement/cleanup paths.
- `ops/planning_export.rs::sync_follow_through_relations` and retained export
  conversion; Linear related-issue helpers used only by that lifecycle.
- Task DTO `follow_through` / `follow_up_sources`, Swift status/due filters.
- `builtins/task/skill/follow-through.md`, `ship.yaml`, `finish-delivery.yaml`,
  and instructions in LOOPFLOW, pr-land, ci-fix, ship-decomposed, capture-tasks,
  task/wave/repo operator and conversation skills that require the ritual.

New remaining work is an ordinary Task plus an ordinary linking comment. Delete
`--none` / `--finish` requirements and dedicated active models. Keep historical
event decoding and unfinished effects until converted without loss. A Task saved
before comment failure must survive; retry only the comment. Neither comments nor
removal of the old gate grants process-control or checkout-deletion authority.

Retained-effect counterexamples (`7ec3ff7dc4`, October 10): historical
provider-UUID intents can predate a local Task and have no recorded network-attempt
boundary. `retain_follow_through_export_in` retains their exact creation ID/input
as attempted and unknown; conversion cannot mint a replacement. An `existing`
intent never creates. Unconfirmed `sync_follow_through_relations` effects retain
their original relation ID/input and confirmation history, not synthetic comment
success. Conversion/retry twice must preserve the reserved local child with missing
link, uncertain provider creation without a local child, and unconfirmed relation
after both endpoints publish. Stable Task/comment identities and unchanged events
are required proof, not executed results. Full source review:
`7ec3ff7dc4:scratch/follow-up-recovery-review.md`.

## 14. Config, cron, naming and public documentation

**Implemented binding cut:** `PmConfig`, `WavePmConfig`, the duplicate provider
selector, legacy-authority gate/cleanup and mixed-authority validation path are
removed. Repository `linear.team` and Wave `linear.initiative` are concrete
bindings; `planning.provider` alone selects transport. Configuration, commands,
current docs, authored goals and surviving fixtures use those fields. Historical
SQL/captures and the one-time `pm.linear_project` selection import retain bytes.
Reteam still confirms remote ownership and preserves delayed observations, but no
longer commits unrelated checkout edits to remove obsolete configuration sentinels.
Provider-shaped acquisition APIs and the remaining PM hierarchy are still required
cuts; this binding deletion does not establish adapter confinement.

The existing pass owns cron unification: one list of ordinary scheduled commands,
existing parser for scope filtering, no parallel Wave-specific cron interpretation.
Preserve identities, timing, Machine ownership, receipts and due accounting without
activating jobs on cloned repositories.

Review `lf/mod.rs`, `lf/commands/planning.rs`, `bin/lf.rs`, builtins, exported skills,
`docs/lf.md`, `docs/architecture/planning.md`, environment and architecture references.
Command ownership should remain coherent; a symbol rename is not a reason to invent
new command families. Remove instructions and examples for mechanisms actually deleted.

## 15. Delete obsolete tests; retain behavioral proof

Conversion-only tests, mock-call expectations, fake recovery fields and mandatory
follow-through ritual assertions should disappear with their production owners.
Adapt meaningful coverage rather than keeping old structures alive for fixtures.

Affected Rust suites include `local_planning`, `planning_exchange_tests`,
`planning_git_tests`, `planning_foreground_tests`, `planning_reconnect_tests`,
`planning_setup_tests`, `planning_lookup_tests`, `task_remote_tests`, `work_watch`,
`dto_fixtures`, `task_follow_through_tests`, `task_flow_launch_tests`, `status_tests`,
plus `ops/pm/` and SQLite unit tests and `pm/linear_follow_through_tests.rs`.
Check root `tests/fixtures/dto/`, including pm_show, planning_sync, local_task,
task_status, task_history_planning and work_frame. Swift DTOFixtureTests,
DesktopHeadlessTests, WorkNavigationTests, WaveLensTests, TaskFlowProofTests and
RegistryQueryTests consume these contracts. Update generated references/export
alignment in the same diff as public removals.

## Proof and stop conditions

Implement in dependency order: canonical Work persistence/reads; direct acquisition
and concrete observations; shared typed edits and journal conversion; projections
and remaining deletion. This ordering follows the existing passes rather than
starting a competing pursuit. Each cut deletes its replaced paths in the same diff.

Before dropping persistent state, inventory the released frontier and current
Task-owned draft. Main merged at `db36f20ba8` materialized LOO-406 and optional
Task PR into `0.13.11.001_release.sql`; `planning_peers.sql` is the sole remaining
Task-owned draft. Main #1523, merged at `c77e72b6b7`, also supplies
`process_terminal.sql`; it owns local terminal placement and stays separate.
Preserve released SQL and edit only the Task-owned draft, with
populated conversion fixtures for Work IDs, legacy external mappings, prose,
criteria, selection, order, uncertain delivery and historical events.

Required behavioral evidence:

1. Git/local and Linear operations return the same domain IDs and typed content;
   no local path depends on provider Markdown or a PM-shaped record.
2. Foreign/unmapped/partial acquisition cannot claim ownership; stale membership,
   deletion, invalidation and relation observations retain their original meaning.
3. Offline Task creation and comments catch up without duplicate effects. An older
   acknowledgment cannot clear a newer edit, reopening or unsent comment.
4. Git conflicts retain competing changes; rejected changes do not falsely become
   applied parents. Journal conversion retains causal and routing history.
5. Imported completion/content leaves each Machine's captured Workflow, Sessions,
   Processes, paths, controls and effect receipts unchanged. Lost responses retain
   exact recovery input; Project rotation and local legacy identities survive.
6. Public remote invocation adopts absent Work from pushed code, reuses it on
   repetition and preserves existing dirty targets. CLI/DTO/headless Desktop
   agree about identity, available cached content and synchronization failures.

Use the pursuit design’s isolated CLI checks and delivery boundary. This audit
claims no new verification or installed acceptance.
