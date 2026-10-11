# One Work model — queued reduction

2026-10-10 · LOO-412 · Jack Heart accepted the structural redesign in
[work-sync-redesign.html](work-sync-redesign.html) and requested this deletion audit.
The exact `WorkEdit` shape belongs to implementation; it requires no further design
approval from Jack. This is accepted scope, with conditional deletions identified below.

## Queue and outcome

Continue the existing passes in [the pursuit design](work-on-another-machine-name.md)
first, including their incomplete Project, follow-up and cron reductions. Then take
this pass in the same Task/PR, subtracting anything those passes already deleted.
Do not expand an in-flight implementation step, launch another writer, or restart
its Flow merely because this document exists. This is the next scope in the design,
not a claim that a runtime job was scheduled. Publish for Jack's next review; do not
land, install or export actual Work into the public repository.

Recovery: the pursuit stopped during sync with `Index was not unstashed`. Both
sides of the scratch conflict and the accepted queue were preserved in `dc7db5be49`;
the working tree then read clean. The retained stash and stopped Flow remain
history. Jack Heart confirmed that implementing this audit is part of the current
goal, not optional follow-up after review.

The result should be one ordinary local reader and typed writer for Wave, Project,
Task and subordinate Task comments. Linear and Git translate changes into those
objects. They retain the evidence needed for synchronization without owning another
writable Work hierarchy. Execution state is machine-local.

## Audit basis and how to use it

Read-only inspection covered definitions, callers, SQL, config, transport, CLI/DTOs,
Swift consumers, fixtures, skills and docs. The tree moved during the existing
pursuit: `28913feeda` flattened plan wrappers; `105e1d15ad` confined Linear comment
observations; `24c377f43f` merged main; `58de9907b6` repaired integration. Paths below
use the post-merge layout under `rust/loopflow/src/` unless otherwise stated.
The earlier HTML diagnosis is pinned to `c8fe4628a`; this audit supersedes its
incomplete account of persistent provider caches.

This is a deletion map, not a promise to delete every named file wholesale. Remove
obsolete responsibilities and all their consumers together. Retain real invariants
under their existing owners. Recheck symbols against the final preceding pass;
line counts would double-count moved code and already-completed cuts.

## 1. Domain wrappers and the missing fields

**Remove:** `PmWave` / `PmProject` / `PmItem` as ordinary domain records.
ProjectPlan, TaskPlan, live Project iteration and public ProjectContent are removed.
ProjectKR, MetricTarget and domain TaskComment are implemented.

**Locations:** `work/project.rs`, `work/task/mod.rs`, `work/task/comment.rs`,
`pm/mod.rs`, `planning.rs`, and their SQL serialization in `store/sqlite/`.

Project persistence has direct description, KRs and targets; the internal
Markdown round trip and public ProjectContent are removed. PM-shaped readers remain.
Task's direct title/description do not yet absorb the shared fields read through
`PmItem` (disposition, assignee, due date and ordering). Complete their actual domain
representation before deleting the old views. Domain ProjectStatus now belongs to
`work/project.rs`. Optional Linear IDs remain bindings, never replacement domain IDs. Keep
optimistic revisions, timestamps, historical iteration events and legacy mappings.
The whole Task struct remains unsuitable for exchange because it contains local data.
At `41378adacf`, `PmProject` also lacks authored description and accepted acquisition
emits no `ProjectDescription` edit. Local persistence/Git fidelity and Linear
in-place Markdown preservation are separate proofs, not end-to-end prose acquisition.

## 2. Delete the second local reading stack

**Removed:** `ops/project.rs::ProjectPlanning`, public `planning_project(s)` reads,
and the Wave/roadmap Project inventory join. Selection, edit/workflow, rotation and
CLI/Desktop Project summaries use canonical Work and explicit Linear bindings.
The canonical list preserves shared rank rather than edit-time order.

**Remaining:** `store/sqlite/plan_read.rs` reconstructs `PmProject`/`PmItem` for
Task observations and `planning_wave`. Delete these with their surviving consumers. `PmTaskRecord` / `PmTaskObservation` must cease being alternate Task
read results, including the embedded Project clone inside a Task observation.

**Evidence:** `project_in` still rebuilds a PM-shaped Project from typed columns;
`task_in` reconstructs
provider-shaped fields and joins provider observations for due date/membership.
`COALESCE(external_project_id,id)` and `COALESCE(external_issue_id,id)` let read
results change ID meaning. Normal `store/children.rs` / `sqlite/children.rs` readers
already provide the other route.

**Keep:** explicit missing, invalid, removed and unavailable evidence. A failed
provider read must not erase cached Work or convert unknown state into absence.
Return canonical IDs with optional bindings and acquisition information; do not
create another generic reader framework simply to replace these functions.

## 3. Collapse accepted-provider-to-Work projection

**Remove or shrink:** `store/sqlite/planning.rs::project_accepted_planning`,
`item_edits`, separate create/update projection branches, and provider hierarchy
readback through `pm_snapshot` / `pm_task_observation_in`. Today `put_pm_project`,
`put_pm_task`, `put_pm_snapshot`, `associate_project`, `put_project` and `put_item`
retain provider records, then copy accepted content into Work. Acquisition should
apply eligible typed facts through the same writer used for local edits.

**Persistent candidates:** duplicated accepted content in `pm_projects`, `pm_items`,
`pm_wave_projects` and snapshot serialization through `PmSnapshot` /
`PmSnapshotRow`. `pm_wave_sync`, `pm_issue_changes`, relation observations and
`pm_project_name_cutover` also need explicit disposition, not blanket deletion.

**Important limit:** these tables retain foreign/unmapped records, partial
inventories, independent relation revisions, invalidation and removal evidence.
An observation cannot become owned Work merely because it was fetched. Preserve
necessary concrete Linear observations/bindings until association is established;
do not recreate the same second hierarchy under new names. Whole-table deletion
is conditional on a populated migration proving every retained fact has an owner.

`require_accepted_initiative`, membership validation, revision ordering and
`reconcile_pm_project_teams` embody real rules. Relocate their responsibilities
into acquisition; do not drop checks for stale independent Team/Initiative facts.

## 4. Remove cache joins from secondary SQL readers

Convert these consumers along with the preceding cut:

- `sqlite/planning_changes.rs::observation`: pending-edit baselines from provider caches.
- `sqlite/planning_order.rs::{observed_order,observe_in,project_members,current_members}`:
  Project order JSON and issue membership from `pm_projects` / `pm_items`.
- `sqlite/task_state_delivery.rs`: provider baseline for disposition delivery.
- `sqlite/project_selection.rs::readiness`: synchronization and Project observations.
- `store/sqlite.rs::repair_wave_repo`: repository moves across Work and provider tables.
- `sqlite/revisions.rs`: update watched domains when owners/tables disappear.

The replacements read Work plus the exact concrete evidence needed by each
operation. Repository repair remains atomic and refuses conflicting identities;
selection still respects unfinished rotation and current provider observations.
An old observation remains old after conversion.

## 5. Delete the internal Markdown storage round trip

**Locations:** `sqlite/project_content.rs::{write_content,update_project_content,
select_project_workflow}`, `sqlite/planning_write.rs`'s
special Project reads/writes and `ops/chapter.rs`. The former `pm/mod.rs` Markdown
functions now live privately in `pm/project_document.rs` for Linear and migration.

Structured persistence and removal of `read_content`/writer Markdown assembly
are implemented. Ordinary operations now take typed fields directly; Chapter input
is flat. The redundant `save_content` forwarding layer is deleted. Migration parses
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
repeated string-field switches, and needless JSON-inside-JSON values for Team IDs,
Initiative IDs and comment authors. The exact enum grouping is implementation's job.

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
This prerequisite does not complete prose acquisition or remove provider caches.

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
read_task_planning_async,inspect_task_planning,pm_resolve_task,resolve_owned_issue,
list_pm_waves,list_local_waves}`, `ops/task_pm.rs::ResolvedTask`,
`ops/planning_sync.rs::find_task`, `ops/run.rs`, `ops/task/remote.rs`.

Use the store's canonical Task selector for local Task ID, provider ID and issue
identifier. Perform selected-transport acquisition where needed, then return that
same Task. Delete overlapping resolution wrappers and PM-context construction
that only selects another domain reader. Retain bounded refresh/lock behavior;
`read_pm_snapshot`, `plan_snapshot_read`, `load_show_snapshot` and
`fetch_pm_snapshot_with_store` need consolidation with the cache removal rather
than new wrappers around renamed snapshots.

Linear setup, OAuth, Team/Initiative mapping and reteam are real integration
operations. Keep them concrete. A local lookup cannot infer that another Machine
has never started work; this rewrite grants no distributed execution authority.
Remote admission must still fetch pushed code and retain dirty or divergent targets.

## 11. Keep necessary delivery evidence without a generic hierarchy

**Locations:** `planning.rs`, `ops/planning_delivery.rs`, `ops/planning_export.rs`,
`sqlite/planning_sync.rs`, `sqlite/planning_export.rs`, `sqlite/planning_order.rs`,
`ops/linear_observe.rs::PlanningSync`.

Separate domain inputs such as `NewTask` from concrete Linear delivery/error/status
types. Remove redundant Project/Task content copies used only for display and
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
canonical Work. Remaining: `PmTaskSummary`, Task status/detail readers and Work
frame Task projections; remove their alternate identity/read paths end to end.

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

## 14. Config, cron, naming and public documentation

`config.rs::PmConfig` and `work/wave/config.rs::WavePmConfig` must stop duplicating
transport selection. Keep one Git/Linear choice, explicit Linear Team and Initiative
bindings, and the default user-keyed ref. Company repository configuration can
select a shared ref or Linear. Rename integration-specific APIs where appropriate;
remove obsolete provider selectors and generic PM scaffolding instead of carrying
aliases. Update config fixtures and Wave frontmatter consumers together.

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
Task-owned draft. `local_planning.sql` came from LOO-406; it is not a spare draft
for this Task. Preserve released SQL and edit the current Task's one draft, with
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

Build the CLI from this worktree, test disposable stores with inherited authority
cleared, and run focused checks per cut; gate owns affected suites once. No source
fixture proves installed or real two-machine acceptance. Structural audit only:
no new product test pass is claimed by this document.
