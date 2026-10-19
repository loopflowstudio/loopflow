# Store structured IDs canonically and accept any casing

Status: accepted outcome; implementation complete pending gate, 2026-10-18. Jack Heart's product direction remains accepted. Pass 6 integrated main through v0.14.7 (`82ae9228fa`), repaired the command proof and finished Session/Linear typing, per-domain Task selectors, history/native selectors, Swift/Desktop CLI transport, docs and the installation proofs. No product decision is outstanding. Gate owns the full Rust/Swift matrix; CI owns the disposable-install run.

During LOO-499 review on October 17, Jack Heart requested case-invariant IDs everywhere, including Linear IDs and normalized storage. Later that day Jack Heart authorized autonomous planning, implementation, verification, publication and landing, superseding the earlier design/demo/merge review boundary. The selected Flow is kickoff → pursue → ship.

LOO-499 / PR #1585 remains independent: preserve its simplified conversation opening and removal of `--choose`; integrate its landed code normally without inheriting unpublished work. The Infrastructure Project workflow is unchanged. Inspection baseline: `28dc1416712c59203ac89d0d4128eb0e7f79b085`. The seed's earlier LOO-499 observations were from `59ce70830126b2e27615e3ae5e7c5456f1c71e17`.

October 18 integration: main advanced through v0.14.6/v0.14.7, releasing
`optional_task_ancestry`, `remove_landing_timeouts`, `session_transport` and
`session_preview`. The draft now has no unreleased dependency, converts and
enforces the new `tasks.repository_id` and `task_creation_intents.repository_id`,
and suspends/restores 0.14.6's repository and membership triggers using their
released text. LOO-499 (#1585) landed its NOCASE/`eq_ignore_ascii_case` ticket
patches; this branch replaced them with domain resolution.

## Problem and demo

People and callers should never need to remember an identifier's casing. Spelling variants must have the same equality, hash, persisted key, references and emitted representation. Comparison-only lookup fixes do not satisfy this outcome.

Demo: in a disposable Machine upgraded from populated released SQL, run `lf task status lOo-506 --json`, repeat with the uppercased stable Task ID and Linear UUID, then inspect that Task's Session using its uppercased Session ID. All variants select the original objects and emit canonical IDs. The Task retains its checkout, PR and Workflow; the Session retains its name, native conversation, captured history and attachment authority. A provider-native ID differing only by case remains a different native conversation. Exercise the same fixture through Desktop's CLI transport and the remote Task API. The ticket is a fixture, not a live Task.

This serves Infrastructure's reliability/continuity objective and its current Project KR that generic Sessions can advance Tasks without plumbing failures. There is no new performance claim or Wave metric.

## Findings that determine the design

1. `id.rs::uuid_id!` validates UUIDs but retains casing. `durable.rs::durable_id!` and `child.rs::prefixed_uuid_id!` require exactly cased prefixes; derived serde and raw constructors bypass parsing. The third macro owns Task PR and landing IDs, so changing only the seed's first two files misses delivery identities.
2. `linear/mod.rs::validated_string_id!` only rejects empty input; serde/raw constructors bypass even that. Initiative, Team, comment, attachment and assignee IDs also travel as strings. Task labels are strings; local `lf-<hex>` labels must remain distinct from uppercase Linear tickets.
3. `LfSession.id` and several Process/Session event references are strings. New Sessions use `session_<hex>`; retained Sessions include bare hexadecimal identities. Captured input selectors also accept `run_` locators through `sessions.rs::resolve_history_input`. Capture locators refer to immutable files and are not Session identities.
4. Task lookup combines stable ID, external UUID, ticket, creation-intent JSON and abbreviated hex. This checkout still compares tickets exactly; the seed describes LOO-499's separate NOCASE patch. History/project/wave queries also compare raw selectors. One CLI patch cannot establish the invariant.
5. An in-memory replay of all 76 registered SQL migrations succeeded; this checkout's released frontier is `0.14.4.001_release.sql`. It contains cyclic references, polymorphic targets, composite keys and JSON identity fields. No installed database or credential records were inspected.
6. `store/MIGRATIONS.md` and `migrations.rs` already provide an exclusive migration transaction, on-disk backup, disabled FK actions during conversion, final FK validation and typed JSON validation. SQL also runs independently during schema generation. Use one self-contained SQL draft; a Rust-only callback would miss that path.
7. Disabling FKs does not disable triggers. Released guards prevent updates to captures, admissions, Flow records, Workflow moves, historical PRs and repair incidents; ancestry/placement triggers observe partially changed parents. Temporarily remove affected guards inside the migration and restore their behavior before commit.
8. Planning mutation UUIDs are string map keys/predecessors. `planning_exchange.rs` rejects reused IDs with unequal contents and breaks equal-clock ties by ID ordering. Normalize keys, parents, targets and typed values before validation/merge. Detect duplicate normalized keys before collecting a map. A changed existing winner is a migration conflict, never an excuse to silently reproject Work.
9. Machine IDs are generated structured `home_<hex>` despite the old opaque-ID comment. October 7 Wave memory preserved their spelling across the Home→Machine rename. The later LOO-506 request changes case only: retain prefix, punctuation, routes, filesystem names and scheduling identity. Cron plist `LoopflowHomeId` decodes through `MachineId`; Swift `WorkspaceIdentity` and `WorkWorkspaceKey` hash strings and require a cache-boundary cutover.
10. Promotion serialization does not prove every old client is quiescent: CLI-only switches explicitly leave prior app processes running. A SQLite lock cannot normalize an old client's in-memory claim. Already-canonical live claims need no rewrite; defer an upgrade that would invalidate a noncanonical live owner until its normal lifecycle settles. Do not kill processes or remint claims. Use existing exact-owner evidence, not PID equality or an uncompleted historical row alone.
11. **Baseline counterexample:** `store/sqlite/processes.rs::session_fence_file` derived the OS fence filename from SHA-256 of the raw Session ID. The fence is held across provider I/O outside SQLite; it can also be acquired before a claim is recorded. Changing the ID changes the fence, even on a case-insensitive filesystem. A disposable probe held the original fence and a SQLite exclusive transaction while successfully acquiring the canonical ID's fence. Checking recorded attachment owners alone therefore does not account for every fence holder. The first slice must account for the original fence before normalizing construction or storage.
12. At the inspection baseline, `journal/mod.rs::read_process_receipt_at` derived the receipt filename from the Process ID. `recorded_process_evidence` then compares the receipt's raw Process and Trace strings with the typed row. Normalizing the row first can lose both the old filename on case-sensitive filesystems and the exact-string match. LF rows deliberately have no `os_started_at`; their receipt supplies the OS birth evidence. The existing typed reader cannot establish pre-upgrade liveness after canonicalization has erased the stored spelling. Trace journal directories are also derived from Trace IDs in `journal/mod.rs::create_process_context` and need an explicit locator policy.

13. `journal/mod.rs::read_process_receipt_files_at` skips unreadable files, malformed JSON and unsupported schemas. Its successful inventory is therefore not complete evidence of absence. Preflight must retain an affected receipt read failure as Unknown; pruning also compares the raw Trace string and needs the same domain-aware matching as liveness reads.
14. Release materialization combines drafts into a migration named `release`, retaining `-- draft: <name>` SQL markers. A new preflight keyed only by `Migration.name == "canonical_structured_ids"` would miss the shipped path. The baseline `migration_preflight` had no guard sink, so acquiring fences only inside that callback would release them before SQL conversion; the enclosing transaction must retain their guards.

SQLite supports targeted [JSON path updates](https://www.sqlite.org/json1.html) and [trigger error messages](https://www.sqlite.org/lang_createtrigger.html#the_raise_function). Bundled libsqlite3-sys sources report 3.53.2, above the 3.47 requirement for expression error messages. A synthetic SQLite 3.50.4 probe confirmed opaque JSON sibling preservation, FK consistency and collision rollback. This proves the mechanism, not the finished migration.

## Chosen representation and authority

SQLite's existing Work, Session and Process owners remain authoritative. Planning journals retain their edits; DTOs and Desktop views derive from those owners. No new identity registry or alias table.

Normalize ASCII case in domain-owned constructors, parsing, serde and SQL conversions. Preserve accepted punctuation and legacy formats: this Task does not equate hyphenated and unhyphenated UUIDs. Keep existing validation contracts; do not turn unrelated historically accepted strings into UUID-only fields as an incidental cleanup.

| Domain | Canonical representation |
| --- | --- |
| Wave, Trace, Process/Flow, steer/batch, attachment UUIDs | Lowercase through `id.rs` types. Normalizing a token never creates a claim. |
| Task, Project, repository, Machine, response, cron, PR, landing | Lowercase prefix/hex. Consolidate the two prefixed-ID implementations around one mechanism. |
| Loopflow Session identity | Introduce `LfSessionId` and propagate through owners/serde; lowercase recognized released structured forms. Preserve accepted historical nonstructured values rather than inventing identities. |
| Linear-owned UUID fields | Lowercase at Linear response, authored binding, model and persistence boundaries, including Initiative/Team/comment/attachment/assignee IDs. Existing non-UUID strings are not presumed structured. |
| Task labels | Domain-owned Task identifier/parser: lowercase `lf-<hex>`, uppercase Linear `<team>-<number>`. Normalize abbreviated hex and prefixes without changing ambiguity/minimum-length rules. |
| Other owned UUIDs | Mutation/edit/comment/delivery UUIDs and predecessor/map keys use the same domain normalization. Inventory by ownership, never `_id` suffix. |
| Opaque values | Native conversation/turn IDs, provider-account IDs, request/cursor tokens, credentials, hashes, URLs, paths, slugs, names, arbitrary content and capture locators retain bytes. |

For multi-domain selectors, try normalized Loopflow identity against its owner; native fallback receives the original selector. An opaque native token resembling an uppercase UUID stays opaque. Names and paths are never lowercased while attempting ID lookup.

Runtime writers normalize before SQL. Add storage constraints/validation triggers for known structured fields so direct or older writers cannot restore mixed-case keys; reject noncanonical writes with domain-specific errors rather than silently rewriting them. Wire shapes remain strings, without renamed fields, defaults or protocol splits. Rust owns remote and CLI ingress. Swift normalizes typed identity/cache boundaries against shared fixtures; provider strings/path keys remain untouched. Callers need not pre-normalize requests.

Absent/error states: absent optional IDs stay absent; malformed/empty inputs retain their domain's error contract; valid absent IDs return not found. Multiple prefix matches remain ambiguous. Invalid stored structured references fail the migration with row/domain context. Unknown payload fields and free text remain unchanged. No UUID-shaped guessing in native payloads, command argv, original imported YAML or captured files.

## Migration and affected consumers

### Filesystem authority and evidence

The first implementation slice retains opaque locators on existing owners:
`agent_sessions.fence_locator`, `processes.receipt_locator`, and
`processes.journal_trace_locator`. Migration copies original spellings before any
identity conversion. New rows initialize locators through insert triggers;
immutable guards preserve them afterward. Receipt lookup and pruning resolve the
stored owner, and new Processes sharing a trace reuse its retained directory.
Receipt writers/readers use the selected ledger's directory. No file is renamed.
Review fixes also bind pruning to the retained receipt owner and include a
canonical attached caller when its claim token/reference changes case.
Receipt path construction, decoding and Process/Trace matching each have one
implementation shared by migration, runtime and pruning; writing and cleanup use
the same path constructor. Tests share receipt setup, and commit/rollback-hook
proofs replace the redundant synthetic conversion-rollback test. Preflight names the affected agent, parent and
attached owners directly; the transaction has one rollback path for migration or
commit failure, with the same retained guards.

`store/canonical_ids.rs` reads raw rows and receipts before typed decoding. It
tries affected Session fences nonblocking inside the exclusive SQLite transaction;
its caller owns acquired guards through commit **and rollback**, including failure
partway through preflight. Dispatchers holding a fence while waiting on SQLite
therefore cause a retryable deferral rather than deadlock. The runner recognizes
`-- draft: canonical_structured_ids` in materialized releases as well as the draft
name; clone preparation uses the same preflight.

An affected attachment includes canonical Session/Process IDs whose token or
provider/attached reference changes. The canonical attached caller must also be
proven dead. Already-canonical live ownership proceeds unchanged. Live affected
owners defer; unreadable, malformed, unsupported or mismatched receipts remain
Unknown even for completed/pre-boot rows. Retry follows normal settlement or
explicit receipt repair; Unknown never ages into permission.

Focused coverage now includes Trace locator collision rollback through the actual
materialized release; mismatched Process and Trace receipts in addition to malformed,
unsupported and unreadable receipts; and OS fence retention observed inside SQLite's
commit/rollback hooks, including partial preflight failure. A canonical live
AgentProcess retains its exact attachment/token/native thread across upgrade and
successfully uses the production dispatch boundary afterward. The pre-upgrade shared
fence still models the released dispatcher; this does not drive a provider socket
across commit. The waiting-descriptor test proves inode continuity and stale
lookup/insert rejection, not an in-flight native send. Tests that manually change
fixture IDs after retaining locators remain filesystem proofs, **not** the full
conversion. Linux gate owns case-sensitive filesystem and isolated installation
acceptance after the completed conversion.

The draft now rejects distinct raw Trace spellings before retaining their different
directory locators. The inventory below includes historical JSON and collision
scopes; their full conversion and rollback proofs remain unfinished. The shared Session domain must match the recognized released forms used
by preflight and storage guards: `session_<32 hex>` and bare 32 hex; other retained
Session strings remain opaque.

The one draft is `migrations/drafts/canonical_structured_ids.sql`, created through `scripts/new_migration.py` with a dependency on the inherited `session_transport` draft. Extend it in place with the complete conversion; never add a second migration or recreate/undo the inherited draft. Published migrations remain immutable.

### Column and JSON inventory (October 17)

Replayed registered migrations through `0.14.4.001_release.sql` in memory, then
audited the drafts including #1586. No installed database was read. This inventory
names conversion candidates by ownership. Wave relational conversion and Trace/Wave
collision checks have populated released-fixture proofs. The other domains, declared
JSON paths and full conversion fixture remain unfinished. Brace notation expands
named columns; no suffix-based rewrite is intended.

| Table | Structured columns |
| --- | --- |
| `waves` | `id`, `parent_wave_id`, `superseded_by_wave_id`, `current_project_id`, `project_activation_lf_process_id` |
| `projects` | `id`, `wave_id`, `external_project_id` |
| `tasks` | `id`, `project_id`, `external_issue_id`, `issue_identifier` (label), `primary_session_id`, `planning_assignee`, `planning_team_id`, `parent_pr_id`, `checkout_machine_id` |
| `repository_plans`, `machines` | `id` in each |
| `agent_sessions` | `id`, `task_id`, `wave_id`, `agent_process_id`, `transport_lf_process_id` |
| `processes` | `id`, `trace_id`, `parent_lf_process_id`, `caller_session_id`, `caller_agent_process_id`, `agent_session_id` (LF Session), `attached_lf_process_id`, `attachment_token` |
| `session_events` | `session_id`, `lf_process_id`, `task_id`, `wave_id`, `agent_process_id` |
| `session_activity` | `session_id`, `attachment_token`, `agent_process_id` |
| `flow_processes`, `flow_process_steps` | `lf_process_id` in each; `flow_process_steps.flow_lf_process_id` |
| `task_workflows`, `task_workflow_moves` | `task_id`, `lf_process_id` in each |
| `task_events`, `project_events` | `task_id` / `project_id` respectively |
| `task_prs` | `id`, `task_id`, `parent_pr_id`, `linear_attachment_id`, `linear_comment_id` |
| `pr_landings` | `id`, `task_id`, `supervisor_machine_id`; **not** OS PID `supervisor_process_id` |
| `ci_incidents` | `landing_id`, `task_id`, `pr_id`, `repair_lf_process_id`, `repair_session_id`; `identity` is `github:ci:<repo>:<PR>:<head>:<failure hash>`; preserve it |
| `task_pr_repair_incidents` | `task_pr_id` |
| `task_linear_observations`, `task_linear_ingested_comments` | `task_id` in each; `task_linear_ingested_comments.comment_id` |
| `task_deletions`, `task_issue_identities` | `wave_id`, `issue_id`, `identifier` (label) in each |
| `pm_projects` | `id` |
| `pm_items` | `id`, `identifier` (label), `project_id`, `team_id`, `assignee`, `claimed_project_id` |
| `pm_wave_sync`, `pm_wave_projects` | `wave_id` in each; `initiative` / Linear `project_id` respectively |
| `pm_issue_changes`, `pm_project_name_cutover` | `issue_id` / `id` respectively |
| `project_binding_imports`, `wave_workflows` | `wave_id` in each |
| `project_transitions` | `wave_id`, `successor_id`, `predecessor_id` |
| `project_transition_items` | `wave_id`, `successor_id`, `issue_id` |
| `task_creation_intents` | `task_id`, `project_id` |
| `task_state_deliveries` | `id`, `task_id`; `target` is a state |
| `task_comments`, `task_comment_deliveries` | `id`, `task_id` / `comment_id` respectively |
| `project_changes`, `task_changes` | `id`, `project_id` / `task_id` respectively |
| `planning_peer_changes`, `planning_peer_observed` | `id` in each; `planning_peer_changes.object_id` is generated from JSON |
| `planning_peer_routes`, `planning_peer_conflicts` | `mutation_id` / `object_id` respectively; destination IDs are authored keys |
| `steer_adapters` | `agent_process_id` |
| `steers` | `id`, `wave_id`, `task_id`, `project_id`, `session_id` |
| `session_steers`, `steer_deliveries` | `session_id`, `steer_id` in each; `steer_deliveries.batch_id` |
| `steer_batches` | `id`, `session_id`, `agent_process_id`, `attachment_token` |
| `task_admissions`, `work_placements` | `task_id`, `machine_id` in each; `work_placements.{wave_id,project_id}` |
| `tool_responses` | `id`, `work_id` by `work_kind`; `request_id` is opaque |
| `metric_instruments`, `metric_observations` | `wave_id` in each; **not** content-hash `observation_id`; original payload/digest retained as described below |
| `linear_team_checks`, `relocated_task_processes` | `team_id` / `task_id`, `process_id` respectively |

`provider_routes.scope_id` is a repository slug (`RouteScope::Repo(RepoId)`) or
empty default scope, **not** RepositoryId or Work. Preserve it. Also preserve
provider/account/session tokens, browser/profile IDs, planning destination IDs,
planning user keys, paths/locators, integer event/sequence/PID keys, revisions,
hashes, names, graph nodes and author/content text. `provider_account_switches.id`
is an integer sequence. Schema history, blob tokens, credentials, profiles, account
limits and route/import delivery evidence are not structured-ID owners.

| JSON owner | Declared identity paths / retained bytes |
| --- | --- |
| `tasks.admission_authority` | `$.machine_id`, `$.repository_id` |
| `projects.{planning_initiatives,planning_teams}`, `pm_projects.{initiative_ids,team_ids}` | Linear UUID array elements |
| `pm_projects.task_order_json` | Local Task-ID array (`planning_order.rs` joins to `tasks.id`) |
| `task_comment_deliveries.{comment_json,conflicting_comment_json}` | `$.id`; body/author unchanged |
| `project_transitions.local_plan_json` | `$.wave_id`, `$.successor_id`; content/graph remains authored |
| `{project_transitions,task_creation_intents}.export_json` | `$.{id,initiative,link_id}`; `$.baseline[*]` WorkValues; known `$.input.{id,teamId,projectId,assigneeId}`, `$.input.teamIds[*]`; no arbitrary input recursion |
| `task_changes.value_json` | WorkValue `$.value` only for `task_project` and `task_assignee`; deletion receipts contain no IDs |
| `{task_changes,project_changes}.{base_json,conflict_json}` | Optional WorkValue under `$.value`; distinguish order Task-ID array `$.value[*]` |
| `project_changes.value_json` | Task-ID array for `field='task_order'`; other current Project variants contain no IDs |
| `project_changes.order_effects_json` | `$[*].before[*]`, `$[*].after[*]` Task IDs; `$[*].issue` Linear ID; `input` contains only `prioritySortOrder`/`sortOrder` numbers (`ops/planning_order.rs::move_input`) |
| `planning_peer_changes.mutation` | `$.edit.target` by `record_table`; `$.parents[*]` mutation IDs; `$.edit.change` WorkValues; generated columns follow JSON |
| `task_events.kind_json` | Variant-owned `$.pr_id`, `$.steer_id`, `$.lf_process_ids[*]` (historical `$.exec_ids[*]`), `$.invocation_id`, `$.{relation_id,source_issue_id,target_issue_id}`; historical `intent`/`link` paths below; `remaining` contains only outcome, evidence and integer check time |
| `project_events.kind_json` | `$.task_id`, `$.steer_id`; nested `$.event` follows Task event variants |
| `pr_landings.retired_supervisor_json` | `$.home_id` is Machine identity; `$.process_id` is OS PID |
| `pm_project_name_cutover.body` | `$.id`, `$.initiative_ids[*]`, `$.team_ids[*]`; all prose/revision/status/metric names retained |
| `session_events.payload` | Artifact/caller keys, imported manifests, native evidence and arbitrary result content stay opaque; only `$.flow_id` for `type IN ('legacy_flow','legacy_flow_review')` is a typed UUID reference; keep receipt keys, cursor graph names, failure text and captured integer sequence |
| `metric_observations.payload` | Original hashed evidence; never rewrite, including its embedded Wave spelling |

WorkValue identity variants: `wave_parent`, `wave_project`, `project_wave`,
`project_linear_id`, `task_project`, `task_linear_id`, `task_assignee`, `task_team`,
`comment_task`; `task_identifier` uses the label domain; `project_initiatives` and
`project_teams` are Linear arrays; delegation values contain `machine_id`; admission
authority contains `machine_id` and `repository_id`. Other variants carry authored
values. External PlanningSnapshot `changes` map keys and predecessor sets need
collision checks before collection, even for equal contents. Equal-clock winner
changes must abort. No blanket recursive conversion.

Historical audit sources: `906576f39d` and `07a3586ac3^` retain the two
follow-through writers. `intent.issue_id` and `link.issue_id` can be local Task
IDs or earlier Linear UUIDs; `intent.project_id` can be local Project or Linear.
`intent.{relation_id,team_id,state_id}` are Linear UUIDs; `link.identifier` uses
the ticket domain. `key` is an authored `--key` (default `follow-up`), and `wave`
is a slug: preserve both. Nested Project `task_observed.event` uses the same
variant-owned paths. `9c7b6828a9^:engine/{invocation,execution,transitions}.rs` and
`durable.rs` establish the retired Flow UUID and cursor/blocker shapes retained by
`0.13.9.001_release.sql`; cursor `repeats` keys are graph occurrence names.
Steer `payload_hash` hashes already emitted text, not the relational identity:
retain it without rebuilding old payloads. CI hashes contain no Loopflow IDs.

Collision inventory (current schema after #1586, including implicit indexes):

* Every affected primary/unique identity: all owner IDs, `projects.external_project_id`,
  `tasks.external_issue_id`, `task_issue_identities.issue_id`, delivery/change UUIDs,
  and `flow_process_steps.lf_process_id`. Trace directory aliases also conflict.
* Composite keys: ingested `(task_id,comment_id)`, repairs `(task_pr_id,kind)`,
  instruments `(wave_id,metric_id)`, tool responses `(work_kind,work_id,request_id)`,
  deletions `(wave_id,issue_id)`, planning observations `(repo,provider,id)`,
  transitions `(wave_id,successor_id)` and items `(wave_id,successor_id,issue_id)`,
  PR `(task_id,sequence)`, peer routes `(repo,destination,mutation_id)`, peer
  conflicts `(kind,object_id,reason)`, Session steers `(session_id,steer_id)` and
  deliveries with `attempt`, relocated `(task_id,process_id)`, and each nullable
  Wave/Project/Task placement key. Opaque key members retain their bytes.
* Partial/expression uniqueness: `session_event_receipt` retains native thread,
  turn, kind and receipt key; primary Wave Sessions apply only while open;
  active Wave `(repo,ifnull(parent_wave_id,''),name)` applies only before retirement;
  pending transitions use `settled_at IS NULL`; current Task PRs use `historical=0`;
  active steer `(agent_process_id,boundary)` excludes cancelled batches. Existing
  path-, name-, provider- and repo/PR-only indexes do not change domains.
* Planning documents: reject normalized mutation-map keys and distinct predecessor
  spellings before BTreeMap/BTreeSet insertion. Compare existing causal winners
  before and after canonicalization, including equal-clock UUID ordering and
  coalescing target spellings. Reject newly conflicting unique Linear claims;
  preserve already recorded conflicts and all losing mutations. Identity arrays
  (Task order, Initiative/Team membership) must not acquire duplicate identities.

Trace locator and Wave owner collisions are implemented. The other scopes and
JSON conversion remain part of the single unfinished conversion cut.

### Content-addressed evidence revision — implemented

**October 17 implementation choice under Jack Heart's accepted outcome:** `MetricObservationIdentity` owns the original
hash input, including Wave spelling. `MetricIdentity` owns canonical Work
association, with a private Wave string constructed/decoded through the shared
UUID case function. Non-UUID historical strings and metric names keep their bytes.
No new table, alias registry, wire field, payload rewrite or rehash exists.

`MetricObservation::metric_identity` projects its association for validation,
history selection and graduation. Hashing still serializes the original evidence
identity. Current producers bind canonical contracts before creating evidence;
direct library reimports accept the original spelling under that same contract.
The store writes contract identity into relational Wave/metric columns and retains
original payload text. Readings use contract identity; evidence remains original.

`canonical_ids_metrics.sql` is populated released data with two equal-time
observations, one qualifying and one incomplete. The actual materialized draft
updates Wave associations while retaining payloads, digests, source/receipt times,
qualification and row counts. Reimporting original payloads repeatedly is a
Duplicate; a newly received original-spelling payload joins the canonical owner.
SQL and reversed in-memory history choose the same original digest, qualify the
same graduated contract, and emit the same canonical reading. Captured Session
payloads survive and their immutability guard is restored. A Wave collision rolls
back metrics, locators and the migration ledger. UUID simple/hyphenated/braced/URN
forms retain punctuation; metric names remain case sensitive.

The draft currently contains the filesystem cut and Wave **relational** conversion
supporting this proof. It does not yet convert declared Wave JSON references or
prevent all noncanonical writers; those belong to the complete constructor/SQL cut
below. The metric representation obstacle is resolved, not the full migration.

Migration sequence:

1. Build temporary domain-specific old→canonical mappings. Preflight owning uniqueness scopes, composite keys, JSON map keys and existing planning winners. Duplicate identities, conflicting unique Linear correspondences or changed winners abort before mutation. Never `OR REPLACE`, `OR IGNORE`, merge, delete or remint. Report domain/conflicting spellings without payload/secret dumps. Preserve the original ledger and backup; retry follows explicit repair.
2. Complete the filesystem/lifecycle preflight above before rewriting identities. Defer an affected upgrade without changing storage; identify the owner that must settle. Canonical live ownership proceeds unchanged. Prove this in the isolated installation scenario.
3. Within the existing migration transaction, drop interfering immutable/ancestry/placement triggers; update owners, references and declared JSON paths. Update changed values only. Restore guards, add canonical storage enforcement, check semantic relationships/JSON and let the runner validate FKs before committing. Remove temporary mapping/error tables and triggers before final schema comparison.
4. Decode retained cron/Desktop/authored binding fields through their domains. Rewrite structured fields on the normal save path while preserving job labels, directories and opaque keys. Re-key Desktop identity dictionaries without dropping panes or selections; detect conflicts before insertion. No capture-file migration or filesystem rename. Public outputs always emit canonical identities even when old input artifacts retain a prior spelling.
5. Normalize peer documents before comparing with migrated journals. Canonical bytes may change the publication digest and correctly mark publication pending; do not forge delivery evidence or rewrite Git history. Old spelling reimports must be idempotent. Conflicting normalized identities remain errors.

Operational boundary: one upgrade transaction with existing backup/lock policy; work proportional to affected rows and declared JSON fields. No recursive arbitrary-blob rewriting or full-ledger scans on ordinary lookup. Use canonical keys/indexes. Failure restores data, constraints and migration ledger together. Tests use disposable Machines and mocked network/provider effects; installation runs only in the disposable-account harness.

Affected consumers include store readers/writers, CLI selectors/history, Task remote routing, Linear observation/mutation DTOs, planning imports/exports, authored bindings, Session capture/attachment boundaries, cron records and Swift identity/DTO/cache readers. Keep all published wire shapes. Docs: relevant CLI README and `docs/architecture-reference.md` describe canonical forms and opaque exceptions.

## Alternatives, deletions and exclusions

* Per-command lowercase/NOCASE fixes miss serde, equality, joins and persistence.
* NOCASE primary keys alone retain noncanonical output and inconsistent JSON/maps. Blanket lowercase or recursive `_id` rewriting corrupts native tokens and evidence.
* Permanent alias registries/dual writes add another authority. Temporary conversion maps suffice and are deleted.
* `Uuid::to_string()` everywhere changes punctuation as well as case; excluded.

Delete — do not maintain:

* Removed: `durable.rs::durable_id!` and `child.rs::prefixed_uuid_id!`; `id.rs::prefixed_uuid_id!` is the one implementation, preserving each domain's error type.
* Removed from metric evidence: shared raw `MetricIdentity` hash/Work representation and direct Wave field construction. Preserve the original `MetricObservationIdentity` hash input and canonical contract association.
* Removed `metrics.rs::select_current` and separate history scans: `MetricObservationEvidence::from_history` selects by `(source_time, original_digest)` while collecting instrument/qualification evidence. Preserve ordering, qualification and released reimport tests on this path.
* Unnormalized `from_raw` and derived serde escape paths. Remove redundant constructors or route required adapters through the domain owner.
* Removed: exactly cased `lf-`/`task_` selector branches, LOO-499's NOCASE/`eq_ignore_ascii_case` ticket patches, `canonical_task_selector` and `deserialize_session`; `TaskSelector` and `LfSessionId` own them. Ambiguity and original native fallback input preserved.
* Tests requiring structured case preservation; replace with canonical round trips. Preserve native-token, capture, ancestry, admission, PR-history and A→B→A fencing proofs.
* Removed in the filesystem slice: identity-derived lookup of retained Session fences and Process receipt files. Preserve the surviving locator path, exact-owner evidence and immutable locator guards throughout the remaining conversion.

Excluded: changing names/slugs/paths, UUID punctuation, legacy prefixes, provider identifier semantics, planning conflict policy, ownership rules, Workflow product behavior, Project defaults, LOO-499 conversation-opening design and live-store experimentation.

## Internal sequence and remaining work

One indivisible delivery; no separate publication of slices. Main #1586 is integrated.

Implemented (checkpoint `6fddd0a021` plus pass 6):

* `id.rs` owns every canonical domain, the single `prefixed_uuid_id!`, `LfSessionId`
  and `TaskSelector`. A selector carries its spelling per domain (stable ID,
  external UUID, ticket label, local prefix); one canonical string compared
  against every domain had uppercased opaque external IDs such as `issue-1`.
* Session references (`SessionEvent`, `SessionBind`, `SessionSummary`,
  `SessionHistory`, `SessionRecord`, `AgentCaller`, `LfProcess` callers,
  `CodexConnection`, Workflow moves, activity/bind DTOs, `TaskSession`) are
  `LfSessionId`; Linear PR attachment/comment, follow-through and Flow IDs
  canonicalize on decode. Remaining `from_raw` calls normalize, so none is an
  escape path; store reads are already trigger-guaranteed canonical.
* Task, Session, history (`conversation_inputs`, native `read_history`) and
  Project selectors bind each domain's canonical spelling; names, slugs and
  capture locators keep bytes. Remote Task source matching uses `TaskSelector`.
* Draft: complete relational/JSON conversion, collision checks and per-table
  storage triggers. JSON traversal skips non-JSON retained evidence (released
  tests insert corrupt payloads deliberately). The draft was generated by an
  unretained script; edit it directly and keep triggers self-contained SQL.
* Swift decodes Session, workspace, Flow membership and Task location IDs
  canonically; native thread/turn and request tokens stay exact. Desktop's
  location request compares canonical identities. Merge repair: Swift's custom
  `SessionRecord` decoder now decodes upstream's `preview`.
* Docs: README and `docs/architecture-reference.md#identifier-spelling`.
  `scripts/test_task_installation.py` runs five canonical-ID proofs.
* Inventory audit (pass 6): every inventoried column is converted and enforced;
  every FK into a converted identity is converted. Preserved by design:
  `ci_incidents.identity`, `task_state_deliveries.target`,
  `tool_responses.{work_kind,request_id}`.

Remaining: gate's full matrix below; CI's disposable-install and Linux
case-sensitive runs. Wave memory note proposed in `questions.md`.

Storage enforcement (narrowed 2026-10-18): every store open validates the full
product schema, so each added schema object costs launch rows. The agent-startup
launch budget (25,000 rows; main 22,212) failed at 31,166. Two causes, both fixed:
`create_process_context` opened an extra ledger per lf process for the Trace
locator (now only for an inherited Trace whose canonical directory is absent,
reusing the open ledger), and 117 added schema objects. Enforcement now omits
what other guards already cover: columns whose only structured values are FK
references (the store's foreign keys match canonical owners exactly), update
triggers whose remaining columns are primary-key members, update triggers on
tables the runtime never updates (`task_events`, `project_events`,
`task_deletions`, `task_issue_identities`, `tool_responses`,
`planning_peer_changes`) or that released guards make immutable
(`task_admissions`), and the duplicate Session-ID pair. 63 objects remain; the
launch reads 24,903 rows. That margin is thin: a further schema object costs
about 42 launch rows. Legacy PID-named receipts again prune through their
recorded Process when no retained locator owns the file.

## Done when

Gate executes checks once against the finished draft tree. Register ordinary integration modules in `tests/integration.rs`; new `canonical_ids` tests must exist and `--no-tests fail` prevents an empty filter passing. Existing runners clear inherited `LF_*`, select disposable Machines and pin the current compiled CLI.

* A populated released fixture upgrades; mixed-case Task, Session, Process/Flow, Wave, Project, repository, Machine, PR/landing, Linear UUID and ticket inputs produce canonical storage/JSON. Normalization is idempotent and equality/hash/map lookup agree. Missing IDs remain missing; ambiguous prefixes remain ambiguous.
* That fixture crosses SQLite → public Task/Session commands → remote Task transport → Desktop DTO/selection. Checkout, PR, names, capture/native history, Work associations, Workflow position and admission survive. Native IDs differing only by case remain distinct even when UUID-shaped. No replacement records appear.
* Primary-key, unique-correspondence and JSON-key collision fixtures prove rollback and useful diagnostics. Restored immutable guards reject the same forbidden writes. Failed/retried upgrades and repeated peer imports retain counts, causal links and losing edits.
* Canonical live claims survive unchanged; mixed-case input finds the current owner; stale release fails after A→B→A. An upgrade that would invalidate an old live owner is deferred without partial changes, then succeeds after normal settlement.

```sh
# Focused implementation proof; gate reuses an identical-tree pass.
uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow -E 'test(canonical_ids)' --no-tests fail < /dev/null

# Gate regression coverage for this broad identity cutover.
uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow --no-fail-fast < /dev/null
uv run python scripts/materialize_rust_tests.py --prepare-cli -- scripts/test_desktop.sh --filter 'DTOFixtureTests|WorkNavigationTests|MultiplexerStoreTests|canonical_ids' -Xswiftc -gnone
uv run python scripts/check_migrations.py
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Expected: all selected tests pass with canonical references, unchanged opaque bytes and no history/authority loss. The new Swift acceptance scenario consumes the materializer's `LOOPFLOW_TEST_LF` while its source checkout exists; it must fail rather than silently skip when that candidate is unavailable in this gate command. Linux CI owns reconnect tests ignored on macOS. Add the noncanonical-live-owner case to `scripts/test_task_installation.py` and run it in its disposable-account CI environment; `LF_HOME` alone cannot isolate installation. Desktop's headless script builds app/test dependencies; no display, dialog or participant is required. Implementation/compress only build changed code and run their focused proof; broader checks belong to gate.

Existing evidence (October 17): the disposable SQLite/`flock`/SHA-256 probe reproduced distinct simultaneous fences for one case-folded Session during an exclusive SQLite transaction and distinct retained/canonical Process receipt basenames. This remains a synthetic counterexample, not production upgrade acceptance. Gate retains full upgrade and isolated installation acceptance.

Review findings (October 17): evidence/Work identity separation preserves hash input
while making association canonical. The Wave SQL conversion keeps all four UUID
spellings accepted by the Rust parser and restores only the four guards it needs
to suspend. The fixture now checks captured-payload immutability after upgrade.
Historical follow-through fields carry both local and earlier Linear identities;
assuming every `issue_id` is Linear would miss retained Task references.
History selection and qualification now share one traversal; ordering still uses
the original digest and ignores unrelated metric evidence before hashing.
Migration preflight reuses `id::canonical_uuid` so its change detection cannot
drift from the retained punctuation/non-UUID policy.

Check result (2026-10-18, pass 6, `1a810652cb`): focused `canonical_ids` plus receipt/trace/journal/prune/launch-budget 113/113 PASS; earlier PR/task/fixture/comment (1393/1395, both failures since fixed), history/session/project/wave/linear/workflow subsets PASS; Swift identity suites 111/111 PASS; fmt, clippy, `check_migrations.py` PASS. Gate owns the full matrix; CI owns installation.
