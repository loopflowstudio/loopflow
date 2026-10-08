# Loopflow owns the plan

Implementation direction for LOO-406 — 2026-10-07, owned by Infrastructure.
Jack Heart selected the complete local lifecycle, echoing Linear APIs and fields
for compatibility with particular attention to ID generation, then requested
“move towards pursuit”. Deliver one coherent PR through publication for review.
Landing, installation, callbacks, Git synchronization and Linear export are outside
this slice. Reconciled on 2026-10-08: the local CLI core is implemented and
exercised in disposable stores; the complete accepted lifecycle remains unfinished
and is not ready for publication.
The older feedback describing mandatory provider IDs and placement is superseded
by storage checkpoint `3615ac424`, local lifecycle checkpoint `b094fc2d4`, and
compression checkpoint `149f47c0e`. Retaining deletion-recovery evidence is an engineering correction
within Jack's preservation constraint, not an additional product approval boundary.

## Direction

Build the single-machine lifecycle first; it remains useful if synchronization
stops there. The reported repeated five-second planning-refresh failures motivate
removing Linear from local work but have not been reproduced in this exploration.

Jack Heart's later leaning is “the git remote as the true 'master'”, allowing
pending local data without overloading the remote. The laptop remains the ordinary
authoring surface: “My mini is always on, but it feels like my laptop should be the
'master'. Dev machines are supposed to be disposable.” A designated SSH server
would tie recovery to that server; a hosted Loopflow service adds an operational
dependency without a selected benefit. Neither is recommended. Proposed Git
publication gives eventual visibility without code merges; that qualification
still needs acceptance.

## Three situations

| Situation | Official plan and writers | What others see |
| --- | --- | --- |
| One developer, several machines | Initially one SQLite store; later one personal Git plan ref, written by the developer's authorized clients | Named authoring machines fetch the plan; a worker receives only its Task and needed context |
| Startup using Linear | Linear owns connected Waves' Projects, Tasks, rank, text and comments; everyone with Linear permission can edit | Loopflow projects the same shared facts; a personal Git plan must not compete for these fields |
| Independent developers in a company | Each has a separate local/personal Git plan and stable plan ID; company code remote need not accept plan writes | Colleagues see shared code/PRs, not another person's private plan unless access is deliberately granted |

Jack's latest tentative direction separates authority and route: when Linear is
connected, planning should go through Linear, while X still routes through the
originating host when online. Direct Linear fallback after host loss remains open.
Connecting a scope means transferring its shared planning authority, not adding a
second master. Personal Waves can coexist. Linear is the recommended collaboration
surface for this series; a Git-backed multi-person product, permissions UI and
notifications are out of scope. Repository access, not a name in a ref, controls
confidentiality. A per-person namespace on a shared remote separates writes but
does not make the content private. Use a separate approved personal plan remote
where company policy permits; otherwise remain local. Do not default-push private
plan material to the code remote.

## First showable slice

Accepted target behavior. Local CLI planning, placement, operation-only workflow
execution and rotation have source proofs. Native agent execution still encounters
a mandatory Linear mapping; hosted delivery and headless Desktop acceptance remain open.

In a repository with no Linear configuration or credentials, run
`lf task create --title "Fix the parser"`. It creates a durable Task immediately. With no
Wave selected, explicit creation provisions a personal `inbox` Wave and ordinary
Project with no chapter or default workflow. Reads provision nothing. Explicit
Wave selection still works. Task creation needs no checkout, agent or remote.

`lf task edit`, `lf task comment` and `lf task status` use SQLite. `lf checkout`
places the Task. `lf project workflow set <project-id> <workflow>` selects the
Project workflow; `lf task run <id>` takes it up and chooses an edge. Work, commits
and delivery use existing commands. In a GitHub-backed
repository, `lf land -c` and later reconciliation demonstrate verified delivery
and completion with no Linear call. Bare `lf land` retains its existing completion
semantics. Without a code remote, local work still succeeds; hosted landing is
unavailable and is never simulated as success.

Desktop shows the same Task and comments, including unplaced Tasks. Personal Wave
creation changes no tracked `wave/` or `.lf/` file. Existing connected repositories
continue to use Linear until explicitly migrated; this slice does not silently
switch their authority or claim cross-machine durability.

## Current system and earlier attempts

Initial inventory: `35e759aaf4e79e7dc8eef2e8d30048f10172b45a`; planning types,
registration and deletion recovery rechecked at `bc6d11661`. The storage cut below
now supersedes that inventory where noted. Integrated #1497 changes harness selection to `--agent`/`-a`
and removes `--ide` app launch; acceptance uses terminal/headless execution and
headless Desktop readers, without restoring that launcher.

- `planning.rs`: Task and Project retain their existing durable IDs; nested plans
  now carry optional `linear_id` mappings and optional provider observation times.
  `Task.worktree` is optional. Planning-only cached issues still live in `pm_items`;
  importing owned issues into durable Tasks remains unfinished.
- `store/sqlite/planning.rs` accepts normalized provider facts and projects them
  into durable rows. The `local_planning` draft makes external mappings nullable
  while retaining uniqueness. Local create/edit/read use the existing Task rows;
  local operations and public projections now select explicit personal authority;
  owned Linear issue import and the remaining consumers still need the cutover. `store/sqlite.rs::task_issue_identity` separately reads
  deletion-recovery evidence; its only operation caller is `ops/pm.rs::delete_task`.
- `ops/task_pm.rs` resolves Team/Initiative ownership and delegates creation and
  completion to `ops/pm.rs`. `pm_create_task_idempotent` searches UUID operation
  markers around a provider create. Connected comments require a Linear observation;
  personal comments now have a transactional local writer.
- `ops/task.rs::create_prepared_task` first reuses an external-issue mapping under
  the Wave lock, then calls `TaskId::new()` when absent. Project ingestion likewise
  reuses a mapping or calls `ProjectId::new()`. Both mint UUID v4; the earlier
  claimed deterministic import mapping from machine work is absent in this tree.
- `ops/project.rs::{ensure,update_plan}` and `ops/chapter.rs::apply_rotation`
  coordinate provider writes. `ops/task.rs`, `ops/task/lifecycle.rs` and
  `ops/linear_observe.rs` connect planning to delivery and steering. Preserve
  these behavioral boundaries when changing the planning owner.
- Historical file planning: `f407deb93` rewrote numbered Wave Markdown from PM;
  `c113ef04b` deleted `ops/ingest.rs`, the mirrors and pull/export/push-diff loop.
  Its predecessor selected local Tasks by filename order. Jack's reason for
  abandoning Tasks in Git was needing a merge to see new work. Those paths are
  already deleted; do not restore them around the new ref transport.
- Asana: `43a1f85ce` deleted the client, HTML conversion, OAuth and exclusive
  tests/scripts after `8f34be5e8` switched to Linear. The single-variant
  `pm/mod.rs::PmProviderKind` routing is now deleted; Linear operations use their
  concrete client. No provider registry is needed.
- LOO-393: `57a92b1c2:scratch/loo-393-checkpoint.patch` preserves the draft
  `task_record.rs`, `task_record_sync.sql` and Linear attachment methods. They
  are absent from this tree. The sketch carried Task/PR identity and agent
  selection, excluded execution, and published content-addressed attachment
  revisions. It is evidence, not a second implementation to revive. LOO-394's
  later machine work remains independent; its historical `lf ssh` spelling is
  superseded by global `--machine`.

## Keystone data and operations

Use the same supported planning vocabulary, field semantics and operations as the
Linear path: issue create/read/list/update, Project management, comments, state,
ordering, assignees and PR links. Local storage implements those operations in
transactions. Reuse the existing Linear-shaped domain values where correct;
audit differences before introducing replacements. Compatibility means preserving
meaning, nullability and relationship IDs, not just matching field names. It does
not require a local GraphQL server or implementing unrelated Linear features.

For each supported field, fixtures must cover both local operations and Linear
observation: title/name, description, identifier, Project/Team relationships,
state category, completion timestamp, rank, assignee, branch name, URL and comments.
Separate authored values from provider observations; local revisions must not pose
as Linear `updatedAt`. Missing external URL/Team/account mapping remains explicit,
not a fabricated Linear value. KRs, targets and workflow preserve their current
Project-content encoding at the Linear boundary. Add Loopflow-specific placement,
execution and synchronization metadata beside this planning shape.

### Identity contract

- Generate a random UUID v4 once for each locally born Task at creation, including
  before checkout. Projects and comments likewise need stable independently minted
  IDs. Never derive identity from title, rank, branch, hostname or a counter.
- Preserve existing IDs byte-for-byte. `durable.rs` already uses UUID v4 inside
  `task_<hex>`/`proj_<hex>`; that wrapper is Loopflow serialization, not a Linear
  UUID. Keep UUID value, persisted spelling and display selector distinct. The
  existing wrapper can stay; bare-UUID wire spelling is not selected by this draft.
- Retain identity before any external effect. An interrupted retry of the same
  creation operation reuses its ID; a separate create with the same title gets a
  distinct ID. Cloning, synchronization and host routing carry the established ID,
  never mint replacements. Use explicit creation identity, not title deduplication.
- External issue UUID and ticket identifier are separate optional fields. Ticket
  aliases can change on Team moves without changing Task identity or branch.
  Importing a Linear-born issue first reuses its established mapping; when absent,
  mint one durable ID and commit it with the unique provider mapping in the same
  transaction. Repeat imports reuse that ID. Conflicting existing mappings need
  explicit reconciliation, not renumbering execution history. Independent-machine
  import convergence belongs to the synchronization follow-up.
- A local UUID does not prove Linear accepts it on creation. Preserve the mapping
  even if Linear assigns a different UUID; provider idempotency remains a separate
  contract. Enforce uniqueness in storage; full-ID conflicts preserve both inputs
  and report the conflict rather than silently replacing a Task.

Proposed ownership sketch (abbreviated fields, not a complete DTO specification;
the supported Linear field inventory above must survive):

```rust
PlanBinding { id: PlanId, authority: Local | Linear(LinearScope) }
TaskPlan { id: TaskId, project_id: ProjectId, title, description, rank,
           assignee: Option<PersonId>, linear: Option<LinearIssueLink> }
ProjectPlan { id: ProjectId, wave_id: WaveId, name, status, krs, targets,
              workflow: Option<String>, linear: Option<LinearProjectLink> }
TaskComment { id: CommentId, task_id: TaskId, author, body, kind, created_at }
TaskPlacement { task_id: TaskId, worktree, workspace_slug }
```

`PlanId` scopes private/shared identities and repository bindings; it is not a
fourth navigation level. Personal Wave objectives, memory, cadence/budget and
instruments belong in the store. Existing explicitly shared repository Wave
files remain Git-owned definitions: retain a reference to that owner, not an
independently editable SQLite copy. Personal definitions resolve from the store;
shared definitions resolve from Git. An explicit scope-qualified lookup resolves
same-named Waves; no shadowing. Personal workflow customization follows the same
rule. Never materialize personal definitions into tracked repository files.
Schedules' *definitions* can travel later; their activation, receipts and execution
ownership cannot. Fetching a cadence must not install a job on a worker.

Move local placement out of mandatory planning creation, retaining existing bytes
and Task foreign keys. One optional placement per Task per machine suffices.
Preserve PR chains, histories, native Session IDs, usage and process authority.
Existing `pm_*` data remains Linear acquisition evidence, never a second local
planning owner. Import planning-only cached issues into Tasks; preserve accepted
revisions, removal evidence and timestamps. Conflicting ownership stays unresolved.

Proposed Rust entry points:

- `create_task(store, scope, NewTask) -> Result<TaskPlan>`: commit identity and
  authored plan together, independent of placement.
- `resolve_task(store, scope, selector) -> Result<Option<TaskPlan>>`: one resolver
  for full ID, unique prefix, existing slug and Linear alias; ambiguity is explicit.
- `edit_task(store, id, expected_revision, patch) -> Result<TaskPlan>` and
  `append_comment(store, id, comment) -> Result<TaskComment>`: transactional writes;
  comments retain named authors and steer/progress provenance.
- `rotate_projects(store, expected_selection, rotation) -> Result<()>`: in Local
  authority, select the successor and move eligible started work in one transaction;
  preserve backlog and existing checkout exclusion. Linear keeps provider recovery.

Use `lf-<12 UUID hex digits>` for the displayed local selector, extending it when
ambiguous; the full ID always works. Store full IDs in automation. Existing
`LOO-*` aliases remain accepted and displayed alongside the local selector when
connected. Connecting never renames a checkout or branch. New branches use
`lf/<full-task-uuid>/<slug>` so people and offline machines need no shared counter.
No collision is silently resolved by selecting one Task.

Task workflow position stays machine-local and still derives local work status.
Portable completion/cancellation is a **planning observation**, carrying its
original decision identity; it grants no cursor movement, process signal or cleanup.
Local completion commits its decision and local workflow arrival together; the
later outbound observation derives from that event, not another editable status.
Inbound terminal planning can stop admission and surface a conflict with active
work, as today. It cannot claim a local Flow reached `end`. Reopening requires
an explicit new decision; a stale offline edit cannot undo completion.

## Callback follow-up — separate design before implementation

Jack Heart proposed a backwards connection from `lf --machine X` to the host task
store. This draft interprets *host* as the originating machine. Explore this before
replicating entire plans onto workers. Jack called pending updates “sorta correct”
but potentially tricky; disconnect policy remains open.

`--machine X` parses and executes on X. Planning uses the origin's PlanBinding;
checkouts, Sessions, Processes, workflows, machine registry and account operations
stay on X. Remote `task create` can author on the laptop without allocating
execution there; `machine add` still edits X. Never route `LF_HOME` to the laptop
or expose SQLite over a network filesystem.

The origin calls its ordinary planning writer: local SQLite, or Linear followed
by accepted observation. Colleagues' independent Linear edits remain authoritative
without the origin online. Queued local text cannot count as a confirmed Linear
write. Direct Linear fallback on X would require separately authorized credentials
and settlement of unknown host writes; it remains undecided.

Candidate transport is an origin Unix socket, served for the invoking `lf` process
and carried over OpenSSH remote Unix-socket forwarding, without an inbound network
listener on the laptop. [OpenSSH forwarding](https://man.openbsd.org/ssh).
Expose typed planning operations with persisted operation IDs and expected
revisions, never SQL or shell execution. An invocation capability scopes access
to the selected plan and conveys no process authority. Nested dispatch retains
that origin; missing forwarding reports unavailable instead of selecting X's store.

Revoke the capability and cancel forwarding on invocation exit, even with a
surviving SSH control master. No resident service or cross-version protocol.
`lf/commands/ssh.rs` has an account-lease broker at the inspected base, but it is
only transport precedent; LOO-413 owns credential routing. Do not couple plan
operations to account leases.

Proposed write/disconnect contract:

- Local writes commit operation ID, mutation and result together. Linear persists
  intent before the call and retains provider readback; SQLite cannot commit that
  external effect atomically. A lost reply is unconfirmed. Reconnect queries the
  same ID; absent results do not permit another submission while the first can
  still commit. Serialize duplicates rather than minting another identity.
- Disconnected edits report unavailable and retain authored text for deliberate
  retry. Cached reads retain observation age. Started work continues under its
  existing rules and records outcomes on X; fresh-planning boundaries stop. Add
  no offline launch, detachment capability, automatic replay or turn/Flow retry.
- Reconnection inspects uncertain writes and outcomes under their original IDs
  and current completion rules. Process exit cannot complete a Task; stale title,
  rank and Project edits are not replayed automatically. Losing X may lose
  undelivered outcomes; losing the laptop still needs publication or backups.
  The callback supplies neither disaster recovery nor independent offline planning.

Acceptance on a blank X: create/edit/comment through the origin and observe one
plan immediately on the laptop. Lose an accepted write's reply, reconnect and prove
one mutation. Show disconnected text retained and an outcome recorded without
claiming plan acceptance. Preserve X's Sessions, workflow and unrelated plans.
Real loopback SSH must prove socket cleanup with a surviving control master.

## Git follow-up: intended contract, separate design before implementation

Extend `PlanBinding.authority` with a Git destination and exact ref
`refs/loopflow/plans/<plan-uuid>`. Destination is independently configured from the
code remote. A dedicated object store writes canonical JSON objects keyed by full
IDs plus a manifest: schema, plan/repository identity and current Wave selections.
One commit updates one plan ref atomically. No SQLite database, WAL or working-tree
snapshot is uploaded. Explicit refspecs fetch this ref; ordinary branch fetches
must not be assumed to include it.

Each local transaction records an operation ID, base revision and pending patch.
A publisher fetches, reconciles and compares the exact previously read remote OID
on push. On conflict it retains local intent and rereads; it never force-overwrites
an unseen head. Unknown push success is settled by fetching the manifest/operation
IDs, without inventing a new mutation. Git documents exact-value leases and
explicit fetch refspecs; host acceptance of this namespace remains unproved.
[Push contract](https://git-scm.com/docs/git-push),
[fetch contract](https://git-scm.com/docs/git-fetch).

Proposed visibility policy:

- CLI: commit locally first; attempt one bounded publication of the pending batch
  before a planning mutation returns. Report “saved locally; publication pending”
  on failure. No resident or work retry is created.
- Desktop: save immediately, publish after five seconds of quiet with a thirty-second
  maximum delay while the app is running and online. Refresh an open plan at most
  once per thirty seconds and on explicit refresh/open. One in-flight sync per plan;
  no network call per token, paint, status poll or unchanged read.
- Offline, closed or suspended: no finite publication guarantee. Retain pending
  edits until the next command/app opportunity or explicit sync. Show oldest pending
  age and last successful observation, not a misleading “synced” badge.

The timings are proposed product policy, not accepted performance targets. Named
machines alone grant no Git credentials or plan replication. Worker dispatch
carries Task ID, accepted plan revision, brief, needed Wave/workflow context and
branch/PR references. It need not fetch the whole plan. Outcome publication is an
explicit planning write through an authorized route; execution stays on the worker.

Reconcile different fields/Tasks automatically against their common base. Concurrent
same-field text edits retain both alternatives for resolution; wall-clock “latest”
cannot safely order offline edits. Comments union by immutable ID. Rank changes
carry expected list revision and before/after neighbor IDs; competing reorders
require resolution. Project rotation compares the selected Project and affected
membership as one unit. Done versus abandoned is a conflict, never a lexical winner.
These conflicts need a small inspection/resolution surface before sync can ship.
No manual code-branch merge is required, but semantic conflicts cannot be wished away.

Only changed authored planning objects, personal definitions/memory, comments and
portable decision/PR references go to Git. Exclude transcripts, progress telemetry,
Sessions, Processes, workflow instances/cursors, locks, tokens, machine paths,
artifacts and binaries. Coalesce unpublished edits; reuse unchanged blobs; no-op
refresh writes nothing. Retain published history initially. This is **small incremental
traffic**, not a bounded-storage promise: history and comments grow. Measure bytes,
object count and requests in fixtures before choosing compaction or retention.
Never rewrite a published plan merely to meet an invented size target.

Losing any worker preserves published planning and pushed code, but can lose its
unpublished code, native conversation and execution evidence. Losing the laptop
preserves every remotely confirmed plan commit; unpushed edits can be lost. A fresh
client restores from the independently configured plan remote, including personal
Wave definitions, without requiring the laptop. A single local-only installation
has only its backup. Losing the remote leaves fetched copies but requires explicit
selection of a replacement authority; never elect a new master automatically.

## Linear follow-up

Recommend full two-way **planning** in connected scopes, including colleagues'
reorders and Project moves. Linear owns supported fields; Loopflow retains local
execution and Wave definitions. Preserve provider revision, transfer, removal and
stale-read rules. API success is not local execution completion.

Connection first previews scope and export, then retains an exact mapping and
per-object creation intent before publishing Tasks, Projects, comments and PR links.
Do not export all private Waves implicitly. Pause new shared mutations during an
explicit recoverable authority transfer; reads/local execution remain available.
Do not declare the switch complete until mappings and provider readback agree.
Afterward Git retains history/Loopflow-only definitions, not a competing shared plan.
Disconnection likewise requires an explicit snapshot/authority decision.

A write-once Linear ID alone does **not** prevent two creators racing before that
field exists. The connection design must prove single issue creation after response
loss and concurrent machines: shared export ownership plus provider-supported
idempotent identity/readback. Existing description-marker lookup is not proof of
concurrent uniqueness. Client-supplied issue UUID support and its retry semantics
remain an API validation requirement; do not assume them from the local UUID design.
Until proved, the exporter must retain an unresolved create rather than retry it
with a new identity. Local identity survives connection; Linear IDs are aliases.

## Delete — do not maintain

The core types now permit absent Linear mappings and checkout placement. Public
personal creation already commits without placement; Task preparation resolves
existing durable Tasks before entering `ops/task_pm.rs`'s connected resolver.
That resolver still requires Initiative/Team ownership. Remaining cutover targets
include native launch's unconditional mapping lookup and connected registration's
placement requirement. Task rows own ordinary external-ID alias resolution.
**Preservation correction (2026-10-07):** retain `task_issue_identities` solely as
Linear deletion-recovery evidence, with its existing reader limited to that
operation. It is not a second Task resolver or current ownership authority.
Removed the single-variant `PmProviderKind` and its context, client and credential
parameters. Linear operations retain concrete `LinearClient` calls, repository
configuration validation and existing snapshot/wire provider values. Personal scope
still selects Local authority from stored ownership; no fake Local PM provider.
Also removed the unused `PlanBinding` projection/reader and duplicate
`edit_personal_wave_definition` writer. Personal definitions now have one document
writer, exercised by the preservation fixture; stable plan IDs remain in SQLite.

Rewrite tests whose sole contract is “every Task needs a Team/Initiative/issue”
in `ops/pm/task_planning_tests.rs` and `tests/task_initialization_tests.rs`; retain
those ownership assertions for connected scopes. Keep provider observation,
chapter recovery and deletion tests: those behaviors remain necessary for Linear.
Do not wholesale delete `pm_items`/`pm_projects` acquisition evidence. The historical
Asana/file/attachment paths above stay deleted. No new parallel resolver or Task type.

### Preservation counterexample: deletion before Task registration

At `e7f8c4922`, `ops/pm.rs::delete_task` records an issue's UUID, ticket and Wave
before contacting Linear. A successful provider deletion can lose its response;
refresh and chapter replacement can then remove ordinary issue/Project membership.
The identity remains necessary to query trash and confirm the original effect.
It does not prove the effect succeeded or authorize another deletion.

`ops/pm/task_planning_tests.rs::task_deletion_planning_recovers_lost_response_without_ordinary_ownership`
covers that sequence and a new-process retry. Its shared fixture explicitly
asserts zero durable Tasks. `task_issue_identities` has no Project ID; neither a
Task nor a complete cached issue is required for this evidence to exist.

An in-memory SQLite experiment applied the canonical catalog through
`0.13.9.002_release`, inserted one Wave and retained issue identity, and passed
`foreign_key_check` with zero Tasks, Projects, cached issues, cached Projects and
deletion confirmations. A Task-keyed alias cannot represent that valid input:
dropping it loses recovery, inventing a Project fabricates ownership, and creating
a confirmation fabricates success. This disproves the unconditional table
replacement in the earlier draft; it does not establish the new lifecycle.

This correction preserves the existing recovery evidence and operation.
Only issues with established Project/Wave ownership become durable Tasks during
import; unresolved provider facts and recovery identities remain evidence until
ownership is established. The `local_planning` draft retains orphan evidence.
The populated migration proof reads retained Task/Project rows through Rust and
compares PR/Session/workflow/recovery rows byte-for-byte. The existing lost-response
operation test passes against the new schema. These are disposable-store proofs,
not installed acceptance. Keep extending this same draft as the lifecycle changes.

## Internal slices and acceptance

**This slice: the local lifecycle keystone, one coherent PR.** Internal cuts:

1. Replace provider-first identity/resolution and mandatory placement; one migration
   draft against the released frontier, edited in place. Preserve all existing IDs,
   aliases, planning-only issues, histories, links and deletion-recovery evidence.
   Retain the recovery-only table above. No intermediate schemas. Include persisted
   Task/Project JSON shapes in the migration audit: typed columns can pass FK checks
   while historical serialized records fail current readers.
2. Preserve Linear-shaped operations and field semantics while adding personal
   Wave/Project provisioning and store-owned definitions; transactional
   create/edit/comment/rotation with optional workflows and no network dependency.
3. Cut CLI, prompts, delivery and Desktop over to the shared reader; prove complete
   local work and retained Linear behavior. Update architecture, DTO fixtures and
   user docs together.

The first storage cut now represents unplaced local Tasks in the existing model.
`Store::{create_local_task,edit_local_task}` commit authored fields and optimistic
revisions transactionally. The caller supplies one `TaskId` across retries; a
`task_creation_intents` receipt preserves the original input even after later edits.
Conflicting reuse fails; separate same-title creation has a distinct ID. Creation
allocates no PR, Session, Process, workflow or checkout. SQLite row readers accept
optional provider mapping/observation and placement; Task status and Wave projections
now expose those absences without requiring a provider.
The store resolver accepts
full IDs and `lf-` UUID prefixes and rejects ambiguity. Stored local display labels
currently use the full UUID; automatic shortest-unique display remains to build.

The draft rebuilds Task/Project tables without changing existing IDs or child rows.
It depends on the integrated `process_names` draft so retained Started triggers use
current table/column names. No second draft was added. Review found that original
creation input must survive edits for retry comparison, and that an absent provider
observation must remain NULL rather than epoch zero; both are fixed.

Implemented local boundaries (2026-10-08):

- A personal plan binds one canonical repository. Wave membership selects Local
  authority explicitly; adding a provider alias does not transfer it. `personal:`
  qualifies stored Waves, including `personal:inbox`; a shared same-name Wave stays
  separate. Reserved-prefix shared names receive a `shared:` address. Root personal
  provisioning is transactional and leaves tracked definitions unchanged.
- Public create/edit/comment/status use the existing Task rows. `--creation-id`
  retains one UUID, emitted before creation; separate creates remain distinct.
  The content-hash provider marker is replaced too. Local retry reads its original
  Project receipt across later edits and rotation. Comments retain UUID, named or
  unresolved author, progress provenance and directional steering.
- First checkout records placement and PR atomically before creating files. New
  local branches include the full UUID. Unplaced status and completion work; local
  workflow admission and completion bypass Linear. Wave and checkout locks protect
  placement; queued creation retains the Wave guard through commit.
- `wave ensure`, `wave edit`, Project workflow selection and plan editing operate
  on personal definitions. Goals/memory feed prompt assembly from SQLite; generated
  curation instructions write back there. Project content reuses Linear's Markdown
  encoding with workflow updated in the same transaction. Local rank is allocated
  within its Project; completed time comes from the retained completion event.
- Local rotation commits destination, selection, started Task membership and its
  receipt together, preserving backlog. Original input survives for retry checking.
  Public fixtures exercise rotation twice, retained workflow/checkout and creation
  retry across that switch. Mixed Local/Linear rotation and cancellation races need
  further proof; a local transaction does not make provider effects atomic.
- The same migration also retains `pm_items.body` and `pm_projects.body` byte-for-byte
  and parses their historical fixture shapes through current observation readers.
  PR, Session, workflow, move and orphan deletion-recovery preservation still pass.
  No historical Task/Project JSON payload owner was found beyond those provider
  observation records; complete supported-field coverage remains below.

Remaining work is the complete accepted lifecycle, in this same PR:

- Finish native agent execution through public Task dispatch and workflow skill
  steps. `lf/commands/util.rs::require_provider_session_process` unconditionally
  calls `TaskPlan::linear_id()` for a Task-bound Session; `human_session.rs` calls
  it at native launch and resume. Flow admission already selects local authority,
  so removing provider access only there cannot fix this boundary. Retain connected
  deletion refusal and exact Session/process authority; exercise launch and resume
  with contained native-provider stubs, including terminal/deleted work.
- Import owned planning-only Linear issues into durable Tasks transactionally;
  accepted ingestion currently updates existing Tasks but inserts no missing Task.
  Reuse mappings and preserve revisions/removal evidence. Cut provider registration
  over to optional placement without losing existing branches. Retain orphan
  deletion evidence; do not infer ownership from it.
- Finish paired supported-field semantics (including rank edits, assignee and
  Project summary), personal workflow customization, nested personal definitions,
  scope resolution across every operation, and shortest-unique display selectors.
  Personal rename/move/delete and mixed-authority operations still need inspection.
- Prove public GitHub publication and verified `land -c`/reconciliation with strict
  stubs, plus refusal of hosted success without a code remote. Local link/copy and
  completion paths select authority, but no hosted-delivery acceptance is claimed.
- Add shared Rust/Swift fixtures and headless Desktop reading/view proofs for
  unplaced Tasks and comments. `RegistryQuery.taskComments` already calls the shared
  CLI, and `TaskReferenceSnapshot.workspace` is optional; neither requires another
  reader. `TaskCommentsProofTests` uses `.requiresDisplay`, so it cannot supply
  this headless proof. Existing DTO tests use a connected thread, not a local CLI
  payload. Finish user/architecture documentation and the concrete local fixtures.
- Extend preservation/concurrency proofs: first-placement filesystem failure,
  creation/rotation races, multiple personal Waves committing together, conflicting
  rotation retry input and late completion. Continue the same draft; run the full
  proposed gate only after the cutover.

Compression review: rotation SQL now exposes its transaction and parameter order;
Project content renders once per destination. The obsolete DTO test requiring
every Task to have a Team now checks explicit null preservation. Connected
ownership assertions remain. The local execution fixture runs `flow show proof`,
then uses an explicit `task move end`; it proves neither a native skill step nor
verified hosted delivery. The no-remote fixture proves local placement/execution,
not rejection of a hosted landing attempt. Those acceptance conditions remain above.

Review corrections: provider mapping never selects authority; creation and rotation
receipts retain original input; a public retry follows its creation Project even
when the Task has moved. Optional checkout is resolved once before Git operations.
Public fixtures use subprocesses with inherited Loopflow/Linear authority removed.
Release's inspected child memory reinforces operation-entry tests and rejecting
unknown stub commands. No hosted effect, installation or complete Desktop outcome
is established by these local fixtures. No product decision blocks the remaining
implementation; the full outcome has not been reduced to this checkpoint.

Select concrete authority at the planning operation boundary. Local lookups must
not enter `resolve_owned_issue` or acquire a provider token. Linear ingestion keeps
revision/removal evidence before projecting the shared reader. Update Rust/Swift
DTOs and fixtures together. The callback, Git and Linear follow-ups above require
separate designs; no follow-up Tasks are filed and machine delivery does not wait.

Proposed keystone gate: `uv run python scripts/test.py --rust --swift --loopflow --e2e`
with new public-CLI `local_planning` fixtures and populated released-frontier
migration cases. Tests use disposable stores with inherited LF authority cleared,
stub providers/GitHub and no installation promotion. Unknown CLI calls must fail
in stubs. Release's child memory records how permissive stubs and tests beneath
the public entry point missed failures: each lifecycle proof must reach the CLI
operation and verify durable readback after a lost reply or restart. The current
`--swift` suite builds the app and runs headless model/view tests; `--loopflow`
compiles the app and UI runners. Display-hosted interaction remains optional and
neither compile-only results nor skipped checks count as Desktop behavior proof.
Expected outcomes:

- Zero Linear/auth calls from create through edit/comment, placement, workflow,
  delivery and terminal readback. Planning-only creation allocates no execution.
- Same IDs/comments appear in CLI and headless Desktop views; private planning
  leaves tracked repository files unchanged, including when a shared Wave has
  the same name. Prefix ambiguity cannot select another person's Task.
- Crash/retry preserves a committed Task when checkout creation fails; rotation
  remains atomic, with started identities preserved and backlog retained.
- Existing connected Tasks retain aliases, PR chains, provider-age/removal facts,
  Session history and workflow position across migration. No old binary opens the
  installed store. Linear failures cannot affect unrelated personal scopes.
- Paired local/Linear fixtures retain supported planning fields and nulls. Creation
  crash/retry reuses identity, same-title independent creates remain distinct, and
  importing twice or changing a Linear ticket alias never allocates another Task.
  Two isolated creators need no shared counter; forced short-prefix ambiguity is
  explicit. Connection with a different provider UUID preserves the local ID.

Git follow-up acceptance uses two independent stores and a disposable bare remote,
then an explicitly authorized real host: delayed push, divergent edits, concurrent
rotation/rank, lost push response, unsupported namespace, separate code/plan
remotes and fresh-machine recovery. Record requests/bytes and prove excluded
execution/credential fields are absent. Local Git fixtures do not prove host policy.

Design review found four consequential near-misses and incorporated their fixes:
nullable Linear IDs without planning-only Tasks; namespace separation mistaken for
privacy; a write-once mapping mistaken for duplicate-safe export; remote completion
mistaken for local workflow arrival. Remaining choices are in `questions.md`.

Check (2026-10-08): `git diff --check` and `lf context --skill realign --json` pass; prose-only reconciliation reuses compression's reported PM (71 passed, 2 ignored), local store (5 passed), CLI/DTO (23 passed), formatting and Clippy results at `149f47c0e`; complete lifecycle and headless Desktop acceptance remain for implementation and gate.
