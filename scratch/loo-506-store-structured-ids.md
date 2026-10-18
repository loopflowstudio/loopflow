# Store structured IDs canonically and accept any casing

Status: representation revision required, reconciled 2026-10-17. Jack Heart's product direction remains accepted; the identity conversion remains unfinished. The inventory exposed a content-addressed metric counterexample: changing its embedded Wave ID invalidates the retained observation identity. The recorded 13-test pass includes that proof and the filesystem slice's Trace collision, ownership and rollback coverage. Constructor/storage conversion needs an evidence representation that preserves the accepted guarantees. This remains one internal slice, not a shippable normalization change.

During LOO-499 review on October 17, Jack Heart requested case-invariant IDs everywhere, including Linear IDs and normalized storage. Later that day Jack Heart authorized autonomous planning, implementation, verification, publication and landing, superseding the earlier design/demo/merge review boundary. The selected Flow is kickoff → pursue → ship.

LOO-499 / PR #1585 remains independent: preserve its simplified conversation opening and removal of `--choose`; integrate its landed code normally without inheriting unpublished work. The Infrastructure Project workflow is unchanged. Inspection baseline: `28dc1416712c59203ac89d0d4128eb0e7f79b085`. The seed's earlier LOO-499 observations were from `59ce70830126b2e27615e3ae5e7c5456f1c71e17`.

October 17 reconciliation: fetched `origin/main` advanced to `070e5bb6a4` (#1586,
LOO-503). Its `remove_landing_timeouts` draft removes four pending-CI columns and
reopens only landings blocked by the retired timeout policy. It also changes the
shared migration runner. Integration remains outstanding: preserve that draft and
policy alongside this Task's single conversion draft, and seed upgrade fixtures
before the complete materialized release batch. No identity requirement changed.
Infrastructure and its immediate Release child's complete goal/memory were read;
Release is the only immediate child with Markdown in this checkout. Release's
operation-entry and isolated installation evidence limits remain applicable.

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
| Other owned UUIDs | Mutation/edit/comment/delivery/observation IDs and predecessor/map keys use the same domain normalization. Inventory by ownership, never `_id` suffix. |
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
directory locators. The remaining migration inventory and collision scopes are
unfinished. The shared Session domain must match the recognized released forms used
by preflight and storage guards: `session_<32 hex>` and bare 32 hex; other retained
Session strings remain opaque.

The one draft is `migrations/drafts/canonical_structured_ids.sql`, created through `scripts/new_migration.py` with a dependency on the inherited `session_transport` draft. Extend it in place with the complete conversion; never add a second migration or recreate/undo the inherited draft. Published migrations remain immutable.

### Column and JSON inventory (October 17)

Replayed registered migrations through `0.14.4.001_release.sql` in memory, then
`session_transport`. No installed database was read. This inventory names conversion
candidates by ownership. SQL conversion, non-Trace collision checks and the populated
fixture remain unfinished. Brace notation expands named columns; no suffix-based
rewrite is intended.

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
| `ci_incidents` | `landing_id`, `task_id`, `pr_id`, `repair_lf_process_id`, `repair_session_id`; identity/hash fields need ownership audit, not folding |
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
| `metric_instruments`, `metric_observations` | `wave_id` in each; **not** content-hash `observation_id`; evidence representation unresolved below |
| `linear_team_checks`, `relocated_task_processes` | `team_id` / `task_id`, `process_id` respectively |

`provider_routes.scope_id` is a repository slug (`RouteScope::Repo(RepoId)`) or
empty default scope, **not** RepositoryId or Work. Preserve it. Also preserve
provider/account/session tokens, browser/profile IDs, planning destination IDs,
planning user keys, paths/locators, integer event/sequence/PID keys, revisions,
hashes, names, graph nodes and author/content text. `provider_account_switches.id`
is an integer sequence. Schema history, blob tokens, credentials, profiles, account
limits and route/import delivery evidence are not structured-ID owners.

| JSON owner | Declared identity paths / remaining audit |
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
| `project_changes.order_effects_json` | `$[*].before[*]`, `$[*].after[*]` Task IDs; `$[*].issue` Linear ID; provider input paths need exact writer audit |
| `planning_peer_changes.mutation` | `$.edit.target` by `record_table`; `$.parents[*]` mutation IDs; `$.edit.change` WorkValues; generated columns follow JSON |
| `task_events.kind_json` | Variant-owned `$.pr_id`, `$.steer_id`, `$.lf_process_ids[*]` (historical `$.exec_ids[*]`), `$.invocation_id`, `$.{relation_id,source_issue_id,target_issue_id}`; historical `intent`, `link`, `remaining` still need released-shape audit |
| `project_events.kind_json` | `$.task_id`, `$.steer_id`; nested `$.event` follows Task event variants |
| `pr_landings.retired_supervisor_json` | `$.home_id` is Machine identity; `$.process_id` is OS PID |
| `pm_project_name_cutover.body` | Retained Linear Project DTO; released identity paths still need audit |
| `session_events.payload` | Artifact/caller keys, imported manifests, native evidence and arbitrary result content stay opaque; historical typed Flow observations still need path audit |
| `metric_observations.payload` | Hashed evidence; **no conversion until evidence/output representation is resolved** |

WorkValue identity variants: `wave_parent`, `wave_project`, `project_wave`,
`project_linear_id`, `task_project`, `task_linear_id`, `task_assignee`, `task_team`,
`comment_task`; `task_identifier` uses the label domain; `project_initiatives` and
`project_teams` are Linear arrays; delegation values contain `machine_id`; admission
authority contains `machine_id` and `repository_id`. Other variants carry authored
values. External PlanningSnapshot `changes` map keys and predecessor sets need
collision checks before collection, even for equal contents. Equal-clock winner
changes must abort. No blanket recursive conversion.

Collision scopes include affected primary/unique keys (including composites), the
`session_event_receipt` expression index, primary Wave Session uniqueness, active
Wave `(repo,parent_wave_id,name)`, active Task PR ownership, Project transition
ownership, active `(agent_process_id,boundary)` batches, and Linear correspondence
claims. Opaque members of composite keys retain bytes. Trace directory collisions
are now rejected by the draft; the remaining scopes are not implemented.

### Content-addressed evidence revision — unresolved

**New counterexample, October 17:** `MetricObservation::digest` in
`work/wave/metrics.rs` hashes the typed payload including `identity.wave_id`.
`observation_id` is SHA-256 content identity, not a UUID. `validate` requires that
digest. `accept_metric_observation` uses it for exactly-once insertion and rejects
different payloads under the same ID. The new test
`canonical_ids_metric_payload_case_changes_its_content_identity` proves that
changing only the embedded Wave spelling changes the digest.

The earlier Metrics inventory incorrectly called observation identity structured.
Classifying the digest as opaque does not solve the embedded identity:
`MetricIdentity` currently serves both hashed evidence and canonical Work keys.
Normalizing its decoder would change the bytes used to verify retained digests;
rehashing would remint observations and could change equal-time selection.
The accepted design requires canonical typed outputs, unchanged hashes/history,
no reminting, and unchanged wire shapes.

Source inspection narrows the conflict: `derive_metric_reading_from_evidence`
already emits identity from the contract, not the retained observation.
`metric_observation_evidence` selects by relational Wave/metric columns and orders
equal-time rows by the original observation ID. Public producer input binds to a
resolved contract before hashing. These existing boundaries support separating
original evidence from canonical association without changing the reading DTO.
They do not yet prove retained observation validation, direct library reimport,
in-memory history selection or graduation qualification after normalization.

Required revision: name the surviving authority for original hashed evidence and
its canonical Work association; specify public output and reimport semantics;
preserve counts, original digests, source times, qualification and equal-time
selection. Proposed direction: retain original evidence bytes on the existing
observation owner and derive Work association from its canonical relational column.
The existing MetricObservation wire/hash contract still needs explicit treatment:
the evidence identity must preserve its hash input, while its canonical Work key
uses the shared Wave domain for validation, association and qualification. The
proposal remains draft until a released-fixture proof covers unchanged payloads,
digests/counts, equal-time selection, repeated original-payload import and canonical
reading output. No payload rewrite, alias table, rehash or decoder normalization
was introduced. This is remaining representation work under Jack Heart's autonomous
authorization; no product requirement has been waived or new approval requested.

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

* Duplicate normalization in `durable_id!`/`prefixed_uuid_id!`; preserve public domain types/error contracts around one implementation.
* Unnormalized `from_raw` and derived serde escape paths. Remove redundant constructors or route required adapters through the domain owner.
* Exactly cased `lf-`/`task_` selector branches and comparison-only patches once shared domain resolution owns them. Preserve ambiguity and original native fallback input.
* Tests requiring structured case preservation; replace with canonical round trips. Preserve native-token, capture, ancestry, admission, PR-history and A→B→A fencing proofs.
* Removed in the filesystem slice: identity-derived lookup of retained Session fences and Process receipt files. Preserve the surviving locator path, exact-owner evidence and immutable locator guards throughout the remaining conversion.

Excluded: changing names/slugs/paths, UUID punctuation, legacy prefixes, provider identifier semantics, planning conflict policy, ownership rules, Workflow product behavior, Project defaults, LOO-499 conversation-opening design and live-store experimentation.

## Internal sequence and remaining work

One indivisible delivery. The filesystem slice exists; canonical identity behavior and its acceptance remain unfinished. No separate publication of this internal slice. The metric representation needs revision before dependent normalization. The scalar inventory is explicit; historical JSON paths and non-Trace collision scopes remain open and can be completed independently. Integration of main #1586's migration/runner changes remains outstanding.

1. Preserve the implemented filesystem cut and its passing Trace collision, real-claim dispatch, receipt mismatch and guard-failure proofs. Extend the fixture with the actual final SQL conversion. In-flight provider transport across upgrade and isolated installation remain required; do not reintroduce identity-derived file lookups.
2. **After the metric representation revision, close construction and storage together:** consolidate normalization; cut over constructors/serde/SQL and Session/Linear/planning string boundaries; author the single draft and populated released fixture. Include trigger restoration, collision rollback, peer reimport, opaque tokens and active-claim counterexamples. Focused new test prefix: `canonical_ids`.
3. Cut over shared selectors, local/remote ingress, history and authored/import boundaries; remove comparison patches. Add real-command/remote scenarios against the migrated fixture. Preserve CLI shapes and LOO-499 connect behavior.
4. Cut over Swift identity/cache boundaries and shared fixtures. Verify selection/panes and opaque bytes through real CLI transport. Update user/API documentation.
5. Run acceptance and review every inventory entry. Unresolved noncanonical consumers remain unfinished, not exemptions.

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

Review finding (October 17): the metric content-identity contract prevents treating every owned JSON identity as a rewriteable reference. Existing contract-derived readings and relational evidence selection narrow the required repair; retained-payload validation and reimport still need proof. The independent SQL change rejects Trace aliases before locator/schema writes and leaves source rows and the migration ledger intact on failure.

Check result (2026-10-17): `uv run python scripts/materialize_rust_tests.py -- cargo test -p loopflow --lib canonical_ids < /dev/null` — recorded PASS (13 tests, no warnings), reused for unchanged code; `git diff --check`, `lf context` — PASS after reconciliation; gate owns broader regression and isolated installation after full conversion.
