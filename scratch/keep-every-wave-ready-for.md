# Keep every Wave ready for work — LOO-366

Implementation design, drafted 2026-10-03; review concluded by Jack Heart before
the October 5 implementation attempt. Jack Heart's accepted product direction
comes from [LOO-366](https://linear.app/loopflow/issue/LOO-366): ordinary Projects
need neither chapters nor a default Flow. The selected configuration/KR boundaries are approved for implementation;
Jack Heart resolved shared configuration ownership on October 5. Base: `12016c6d6`.

October 4 direction from Jack Heart: every Wave has exactly one active Project,
independent of chapters; tools and skills coordinate optional repository-wide
rotation through the Wave hierarchy. Jack subsequently specified that the Wave's
Project must be an explicit configuration field, suggesting
`wave/<wave>/config.yaml`. This supersedes status-based current selection and the
inferred automatic sole-candidate activation rule. Jack also requested a simple core API: validate the configured Project and access,
or create one when no Project is configured. Name matching and candidate selection
are excluded from the core. The field layout below is the selected implementation choice; recovery mechanics
remain proposed until implemented and verified.

Jack Heart further selected KRs as the chapter creation boundary: chapter review
and planning generate KRs before creating chapter Projects. That planning may
also generate Task candidates, but adding them is a separate step, optionally
run globally immediately afterward. A complete future Task plan is not required.

## Shared local configuration ownership — accepted October 5

The superseded checkout-owned configuration design and its stale-checkout/duplicate
creation counterexamples remain at
`e04c83513573cc09883fb2b92ebdb63e06a22c95:scratch/keep-every-wave-ready-for.md`
and `/tmp/loo366-before-cold-ownership.md`. Source inspection found checkout-local
policy reads, not the previously claimed remote-main resolver. Canonical registry
identity and a Wave lock do not publish a binding to other checkouts; settled
receipts cannot become a second selector.

Jack Heart selected one shared local Wave configuration for the current Project
binding in Linear comment `f092d63a-a152-4920-af81-d676a576f694` on October 5.
This resolves the ownership decision above. All checkouts resolve the same file
through durable Wave identity; stale checkout files never select or overwrite
that binding. Git publication and checkout synchronization are not prerequisites
for opening or rotating a Project. Recovery receipts remain mutation evidence.

Implementation choice for this revision: `<Home>/waves/<WaveId>/config.yaml`,
resolved through the existing Home path owner and registered Wave ID. The file
holds `pm.linear_project`; it is independent of Wave slug and checkout location.
This path is an implementation choice, not a path specified by Jack. Goals,
Initiative binding, schedules and other authored policy retain their existing
owners; this change does not relocate all Wave configuration. No binding is
read from checkout `config.yaml` or `GOAL.md`, nor inferred from SQLite snapshots
or settled transitions. Missing files mean unconfigured; unreadable or malformed
files remain errors, never permission to provision another Project.

The shared reader is observational, including when the file is absent. Explicit
ensure, binding setup and rotation use the same Wave lock and atomic file writer.
Re-read the expected binding before replacement, preserve unrelated authored
bytes, and durably replace the file before settling its receipt. A crashed write
must expose either the previous or next complete binding. The lock lives outside
the replaced file, so atomic rename cannot create a second lock owner. Ordinary
opening consumes the shared binding even when its checkout contains an older ID.

Optional Flow and the cached-name cutover below are implemented locally. Shared
binding, exact-ID ensure, rotation recovery and Desktop remain unfinished. No
provider mutation or configured acceptance has occurred.

## Intelligence repair — Jack Heart's October 5 steer

Jack Heart's comment `e4dafef5-2a87-4359-818a-3770356ba850` reports Intelligence's
existing Project `999bdbdd-c045-41a6-8ffc-a97c4a40b0b3` (Chapter demo-20260924)
as Backlog with empty Flow, and `lf repo refresh intelligence` rejecting adoption
for missing default Flow. These are supplied incident observations, not a fresh
provider read in this reconciliation. Jack authorized autonomous repair without
another review Session, preserving this Project's KRs, Tasks and identity and
forbidding a competing Project or repository chapter rotation. This supersedes
the earlier review wait for this repair, not the preservation or acceptance tests.

The exact ID supplies the explicit bootstrap selection required by the accepted
configuration policy. The intended supported path is binding that ID through the
shared configuration writer, then status-only activation through ensure and
provider readback. Binding setup needs an explicit supported operation alongside
ensure; its CLI spelling remains an implementation choice. It must validate the
exact Project's ownership under the same Wave lock and preserve unrelated file
bytes. Ordinary ensure must not infer this binding from candidate discovery.

Source inspection confirms `migration_adoption_accepts_a_started_project_without_flow`
uses Started status and disables promotion. It proves neither Backlog activation
nor the configured Intelligence repair. Existing migration adoption can promote
only from retained migration evidence; it is not a general bootstrap API.
The empty-Flow refusal is removed in source, but shared binding and ensure do not
exist yet. No installed repair is established, and a branch binary must not write
the installed Home. Delivery must report the actual supported binding/activation
commands once implemented and available in a published installation.

Add the Backlog/empty-Flow case through the operation entry point: explicitly bind
an existing Project, activate that UUID, preserve original content/KRs, Tasks and
execution identity, and repeat without creating a Project. Configured acceptance
for Intelligence has this explicit destination and repair authority; other fixture
Waves still need designated destinations. No global reset is authorized.

## Cached-name cutover — approved and implemented October 5

Jack Heart approved one-time historical name-only replacement in comments
`5419b87c-bfec-4f42-8914-021483249895` and
`fb2d9c94-ad0c-4762-8658-213734ddc5c2`. Earlier counterexamples and the unresolved
policy are preserved at
`f93638e4e69116e575d98f9b5482774fcda5b93c:scratch/keep-every-wave-ready-for.md`.
A cached plain name can represent either a stripped provider name or an exact
provider name; no migration can distinguish those histories. Jack authorized
accepting either once, while retaining the original evidence.

The Task's one draft, `project_readiness.sql`, archives pre-cut Linear bodies,
acquisition timestamps and archive/membership flags. The body retains revision,
name and slug. Only those rows receive an unconsumed conversion marker. The first
accepted observation may replace name/slug at the same revision; every other
field retains existing conflict checks. An older observation or rejected write
cannot spend the exception. Planning and marker updates share their existing
transaction. New rows and converted rows reject later equal-revision conflicts.
Repository-alias repair carries the evidence in the same transaction as planning.

Task inspection/resolution and Wave snapshots now keep provider names. Sync and
reteam no longer plan or apply prefix renames; the unused rename API and reteam
wrapper are deleted. Creation's explicit naming remains distinct. Ordinary
Project synchronization and recording moved from `chapter` to `project`.
Snapshot and detail ingestion now project accepted entity rows within their SQLite
transaction. Each entity retains its own acquisition time. The separate
`sync_projects` replay and `update_task_plan` API are removed; preservation tests
use accepted observation ingestion. Rotation and reteam now accept confirmed readbacks. Shared acquisition ownership
survives queued SQLite workers; reteam reconciles confirmed Team relationships.

Focused cutover tests cover the indistinguishable histories, already exact names,
original body/time retention, rollback, older acquisitions, non-name conflict,
new rows and subsequent rejection. Lookup and reteam tests preserve ordinary
names. These are local synthetic proofs, not installed conversion or configured
Wave acceptance.

## Remaining coherent cut

Registration selects accepted issue ownership, content and acquisition age within
its insertion transaction, rejecting invalid, removed or terminal observations.
Both public APIs return the committed Task for publication, checkout finishing and
execution. They now share one SQLite insertion path, including admission, optional
initialization event and readback. The queued writer receives the existing Wave
guard by ownership; cancellation cannot release it before commit. Planning is accepted
into that owned Task without another full Task copy. Reserved Task/PR/
checkout identity and initial facts without a cached observation remain intact.

Rotation takes ordered Wave locks, enumerates exact Task roots and acquires one
checkout lock set before classification. Queued writers retain both scopes through
commit. Failed reset releases exclusion so starts can retry. Automatic backlog
cancellation is deleted; unreviewed backlog remains historical and unresolved
abandonment needs explicit settlement. Hierarchical admission covers missing roots
without scanning Task population and permits sibling progress. Git leases remain
separate. Detailed registration, rotation and admission evidence remains at
`7e7204ddad0092beb1a633676f5d44af0853a9a8:scratch/keep-every-wave-ready-for.md`.
These proofs establish neither configuration-switch recovery nor readiness.

The branch is not releasable. Steps 3–7 still own shared binding, exact-ID ensure,
transition recovery, KR-first rotation, Desktop and configured
acceptance. Replace `sqlite/children.rs::insert_initial_task`'s Started-count selector
alongside `require_current_task_chapter`, `resolve_project_for_task` and rotation's
provider-name comparisons. No further cached-name policy decision is needed.

### Accepted planning must own durable projection — October 5

Collection prerequisite corrected October 5: the reviewed instruction to retain
the listing Project's association contradicted the issue's own provider facts.
`fetch_pm_snapshot_for_projects` overwrote both `project_id` and its display slug
after `list_items` decoded them. A moved issue therefore acquired the predecessor
ID with its newer provider revision; a detached issue acquired invented membership.
Atomic projection would faithfully persist that corruption. The loopback operation
regression `fetch_pm_snapshot_preserves_moved_and_detached_issue_ownership` failed
with `project-123` instead of `successor`. Removing the overwrite preserves both
reported ownership and revision, including null membership. The original collection
test and the empty-Flow migration-adoption test also pass. These are synthetic
collection/adoption proofs, not concurrent provider or durable-transfer acceptance.

Collection preserves issue-reported ownership and rotation accepts confirmed
Project/issue readbacks before durable projection. Reteam now uses full readbacks
and explicit Team acceptance; both identifier-only writers are deleted.
Created-successor recovery stays with shared binding and exact-ID transitions. A reported destination outside the
listed Projects still needs explicit ownership resolution, never reattachment to
the requested collection. These source changes do not establish configured recovery.

The five store counterexamples and their exact regression names remain at
`f2b127d87bc0bb99654ba66118fb91a77099b73f:scratch/keep-every-wave-ready-for.md`:
delayed transfer, interrupted rotation transfer, restart restoring old status,
entity acquisition age and reteam identifier rollback. Keep their coverage;
operation recovery remains distinct. Full readbacks now replace identifier-only
writes, and accepted observations replace snapshot replay.

Accepted observations and durable projection share one SQLite transaction.
At `b3d8a6366`, full and partial Wave ingestion validate the accepted Initiative
before membership/freshness writes: stale responses cannot leave a fresh empty
Wave after newer foreign facts are retained. The subsequent cold-detail repair
resolves configured ownership before projection and removes both shortcuts.
Snapshot and detail acquisition now retain shared Wave guards inside SQLite
workers. Cold discovery re-reads ownership under the guard; reteam and rotation
acquire participating guards in stable ID order. Provider calls stay outside
SQLite; acquisition time does not order provider revisions.

**Writer inventory and required replacement:**

- Full refresh and detail ingestion now accept and project within one SQLite
  transaction, selecting the stored body and acquisition time for each supplied
  entity. Projection failure rolls back acceptance. No cached snapshot replay
  remains. Acquisition and ingestion now share the Wave boundary, including held
  locks. Registration now selects accepted issue facts within its insertion transaction.
- Task-update reconciliation no longer writes a resolved snapshot or captured
  Task plan. Detail refresh now resolves the exact Initiative through configured
  Wave ownership before atomic acceptance/projection. Both durable-identity
  shortcuts are deleted. Cold same-Wave refresh, foreign/unmapped preservation
  and Task/PR identity have focused coverage. Cold detail re-reads after acquiring
  the Wave lock and retains that guard through acceptance.
- Restart no longer copies `ResolvedTask` plans into durable Project/Task rows.
  The unused generic `update_task` API and `TASK_UPDATE` are deleted; restart
  retains its Flow retirement, timestamp and event writes. Agent choice and PM
  writeback retain their existing owners. Generic `update_project`, its SQL and
  `record_project` are deleted. Registration now selects accepted issue facts in its transaction.
- Rotation persists full confirmed transfer readbacks and Project status through
  accepted ingestion. `move_chapter_task` is deleted. `put_pm_project` accepts one
  Project, associates its confirmed Wave and projects it atomically without
  claiming a complete Wave refresh. Acquisition time is captured before each read;
  `find_project` now requests the previously omitted provider revision.
  Existing-successor interruption/re-entry preserves Task/PR/Flow identity. Created
  successor recovery still fails under the old name selector; see the sequencing
  correction below. Automatic backlog cancellation is deleted.
- Reteam accepts full exact issue readbacks after both moves and identifier
  reconciliation. Its SQL Team reconciler preserves independent entity revisions
  and timestamps and never authorizes Initiative replacement. Expansion and
  narrowing require exact expected Team readbacks. All participating Wave locks
  precede provider acquisition and remain held through accepted projection.
  Each SQLite worker retains its Wave guard after cancellation. Task creation and
  plan editing reuse their held guard for refresh instead of reacquiring it.

Project and issue bodies have independent acquisition times. `pm_snapshot`
returns maximum Wave sync time while omitting entity row times;
`pm_task_observation` joins a Project body to the issue's `observed_at`.
Preserve each accepted entity's own time through projection; substituting the
snapshot or joined issue time for the current clock remains incorrect.

**Required proofs across the coherent cut:** retain all five cases;
add operation-entry restart versus refresh/rotation, delayed Task-update content
and age, reteam interruption/re-entry with cached planning, and rotation exits
after provider confirmation but before persistence and before final refresh.
Cover both full and partial ingestion, already-held locks, Task creation/update,
independently aged detail entities and an older provider response after rotation.
Store-level proofs alone cannot establish operation recovery.

### Created-successor recovery changes the cut order — October 5

`rotation_recovers_a_created_successor_after_confirmed_transfer` enters through
`rotate`, creates the successor, and loses the connection immediately after the
transfer readback. Accepted planning, Task checkout, PR and Flow survive. Retry
fails with `competing current chapter names: A — next, A — previous`. The same
fixture with an existing exactly named successor completes on retry.

This invalidates requiring all rotation recovery proofs before replacing selection.
Preserving provider names exposes `plan_rotation`'s name-based target/predecessor
inference and the final name comparisons. Do not restore stripping, rename provider
Projects or patch that deletion target. The accepted shared binding and exact-ID
transition receipt must replace the selector before the creation proof can pass.
Writer deletion can precede that replacement, but it is not an independently ready
rotation cut. Finish the remaining membership work;
then implement binding, receipt and rotation together before judging recovery.
No new product decision or reduction of acceptance is implied. The new regression
remains failing under the old selector; the original reteam regressions now pass.
The two-Wave backlog proof also failed on `A — previous, B — previous`. Its focused
fixture now uses matching predecessor names to isolate preservation; differing-name
rotation remains unproved and requires the planned selector replacement.

The rejected global fence’s 31-test pass does not apply to this tree. The
preservation reader and its operation proof are described below. Inspection Execs
and unexecuted Flows do not count as started work. Shared binding, ensure, KR-first
rotation, Desktop and installed Intelligence repair remain unfinished.

### Preservation boundary — October 5

Stabilize Task/checkout membership before changing selection. The earlier locking
proposals and full interleavings are retained at
`2a9fe32076f31512299e19e07235ac07d10857fe:scratch/keep-every-wave-ready-for.md`.
The retained counterexamples shaped the admission and rotation boundary:

- **First starts and rotation share checkout exclusion.**
  `create_session` locks its execution and associated Flow workspaces plus its
  explicit/inherited Task checkout; `bind_session` locks its stored workspace and
  destination Task; `begin_flow_operation` locks before its Started write.
  Canonical workspace and explicit Task paths lock exclusively; shared ancestor
  locks cover even unregistered missing roots. Lock acquisition precedes SQLite.
  Released conversation triggers still own Started, and historical binding stays
  allowed. Rotation now holds the same exclusion through classification and provider
  reconciliation. Configuration-switch proofs await the shared binding.
- **Started is not complete work evidence.** The earlier `chapter_task_evidence`
  read only Started and the managed worker claim. An unbound, non-primary conversation
  under a Task checkout can have neither, even without concurrency.
  `reserve_session_in` correctly leaves its attribution null. Classification
  now reuses shared Task-work Session membership and started mechanical Flow
  history, including primary-scope exclusion. That closes the static reader gap;
  rotation now stabilizes membership with checkout exclusion. A second path counter
  or explicit-ancestry lock would miss this contract.
- **Registration changes membership without a Session write.**
  `ops/task.rs::create_prepared_task` resolves a Project before existing-issue
  registration via `Store::create_task_with_worktree`. The caller holds a Git
  worktree lease; the SQLite worker acquires checkout admission through commit.
  Registration retains the Wave planning lock and
  selects accepted issue facts transactionally. Earlier taskless conversations
  become Task work on registration; rotation enumerates roots under the same Wave
  boundary. The old Project status/count selector remains for step 3. New-issue
  `pm_create_task_idempotent` holds the Wave lock.

October 5: the executed `input_replacement_retains_workspace_and_task_membership`
regression exposed another writer. `replace_session_input` locked only the supplied
destination and `replace_input_in` changed `cwd`; an unbound conversation vanished
from its Task while the source checkout lock was held (expected one Session, got
zero). Production callers replace input without relocating the conversation.
The unused relocation write is now deleted: replacement locks the stored workspace
and uses it in the new capture. Primary workspace admission keeps its separate
writer and excludes bound Tasks. The regression covers exclusion, retry and retained
unbound membership. This is a Session-store proof, not rotation/start acceptance.

October 5 admission proofs cover unchanged Session/Flow bytes while excluded,
retry, missing paths, binding from elsewhere and bound/taskless Flow membership.
Historical completed-Project binding still passes. These are store proofs, not
rotation; the new operation test adds both orderings and failed-reset retry.
October 5's admission audit corrects the Flow concern: `insert_flow_in` omits
cwd for Task-bound Flows, and `FLOW_SELECT` resolves the Task checkout. Claims,
agent-step reservations and review reservations therefore already exclude that
checkout. A temporary four-case probe passed; no production Flow repair was needed.

The separate bound-Session regression failed: input replacement changed a capture
while the Task checkout was excluded. Replacement now uses stored Session and Flow
workspaces plus its bound Task root. Tests retain unchanged captures while excluded
and successful retry for explicit Task binding, Task-bound Flows and taskless Flows
whose workspace associates them with a Task. CI repair's outer admission now also
includes its explicitly bound Task root; `reserve_repair` does not reacquire it.
The CI operation-entry proof remains with gate. These earlier repairs alone did not establish rotation exclusion; the new operation
regression exercises both orderings with an unbound checkout conversation.

The operation order is Wave planning locks, checkout admission locks
in canonical-path order, then SQLite writes. Registration and rotation share the Wave
boundary; rotation enumerates roots under it. Execution needs no Wave lock.
Hold those roots through classification, transfer and the configuration switch.
Rotation submits all roots to one admission acquisition, merging ancestor modes
before locking; separate acquisitions can conflict with held nested-root locks.
Shared ancestor admission already covers taskless work below roots absent when
its admission began. Resolve accepted issue ownership and the selected Project
within registration's transaction; reuse any caller-held Wave guard. No population
lock or root-discovery retry belongs in execution admission.

Execution starts acquire affected checkout locks before SQLite and never wait
for Wave planning locks. Cover direct Session creation, review reservation,
mechanical Flow starts, taskless starts through cwd, and binding from elsewhere.
Include the execution and explicitly bound Task checkouts, deduplicated in the
same order. Resolve omitted ancestry before locking and revalidate it in the
write. Missing paths retain their canonical ancestors without consulting Task
population. Acquire no planning or checkout lock inside a store mutex or writer
transaction.

Final classification combines complete Task-work membership with Started,
authored and publication evidence. Preserve independent Sessions/Flows and their
bytes; membership grants no signaling authority or attribution rewrite. Inspection
Execs still do not establish Started.

After a failed reset, released checkout locks permit ordinary starts. Recovery
reclassifies while the predecessor remains configured. After the configuration
switch, retry reconciles selected transfers and predecessor completion without
sweeping new historical bindings into the successor. Pending receipts alone never
deny starts. Prove both start/rotation orderings and response loss on both sides
of this boundary through operation entry points, retaining actual failures.
Release's recovery findings reinforce this requirement; its publication and
installation evidence supplies no Project-readiness acceptance.

Existing public-store regressions establish narrower preservation contracts:

- `session_binding_retains_a_task_in_completed_project_history`: historical
  binding succeeds, sets Started and retains Task/checkout identity.
- `checkout_session_membership_survives_project_transfer_without_binding`:
  transfer retains unbound Session bytes, checkout identity and membership.
- `checkout_exclusion_preserves_registration_and_retry`: both registration APIs
  retain earlier Session bytes/membership and unset Started after exclusion/retry.
  It also covers the former standalone earlier-conversation registration case.

These earlier store tests prove narrower contracts. The new rotation operation test
adds exclusion and failed-reset retry; configured-ID selection and transitions
remain unimplemented. Jack Heart's explicit-binding policy is unchanged.

## Outcome and demo

Every Wave has one explicitly configured current Project, normally In Progress.
Opening a Wave in Desktop ensures that Project independently of any chapter. Existing
names, content, Tasks, checkout, PR, Session and Flow identities survive. CLI and
agents can do the same with proposed `lf wave ensure <wave> --json`. Status,
roadmap, background refresh and Project inspection never initiate provisioning.
An unavailable sibling Wave or global reset does not prevent ordinary work here.

On opening, show retained Project and Tasks while preparation runs. With no
retained plan, show “Preparing Project…” until acquisition resolves. An outage
shows its cause and Retry, never an empty-project prompt. The configured ID
selects the current Project even if another provider Project is In Progress.
Other Projects do not block opening the configured one. Project
preparation and the primary conversation have independent outcomes. Reopening
retries ensure; periodic refresh only reads. Key results to the opened Wave so
a late response from Wave A cannot replace Wave B's displayed plan.

Demo: open a configured Wave with no Projects, see its ordinary Project and
usable Task section, close/reopen it, and get the same provider UUID. Open a
second Wave whose ordinary Project has no `flow:` line and a name such as
“Summer work — customer requests”; that exact Project remains current. CLI ensure
returns the same records. A failed global reset stays visible under an explicit
“Realign Projects…” action while both Waves remain usable. Task creation and
completion without a managed Flow are LOO-367's integration responsibility;
this Task removes their Project-level prerequisites.

This directly supports the supplied KR about creating and advancing Tasks without
plumbing blockers. No numeric metric targets were supplied; do not claim a KR
from synthetic tests or readiness alone.

## Implementation anchors

Selection and writer replacements are listed under **Delete — do not maintain**;
existing failure sequences and lock requirements are in **Remaining coherent cut**.
The following surviving capabilities constrain their replacements:

- `work/wave/config.rs` reads authored policy from `GOAL.md`; only the Project
  binding moves to the Home-local file described above.
- `ProjectContent` retains its empty-string Flow representation. LOO-367 owns
  validation when launching an explicitly selected or configured Flow.
- `ops/pm.rs::checked_projects_with_store` retrieves retained IDs omitted from
  membership and validates ownership; keep those checks while preserving names.
- `create_project` accepts a chosen UUID, but Initiative attachment is a separate
  mutation. `find_project` distinguishes archived records from absence. Recovery
  must handle an unattached Project without creating another identity.
- LOO-364's `ops/human_session/primary.rs` reserves identity before launch and
  reuses it on retry. Reuse that principle without Session storage or startup.
  `chapter::rotation_lock` supplies a per-Wave OS lock with a 30-second bound.
- Linear's [GraphQL schema](https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql),
  inspected October 3, accepts a caller UUID and optional content/status on create,
  and status-only updates without a revision precondition. This supports exact-ID
  reconciliation, not cross-mutation atomicity. Duplicate-ID and attachment
  behavior still need configured proof.
- Desktop's `WaveDetailPane` polls every 30 seconds and names its ordinary plan
  `chapterAndTasks`/`WaveChapterView`. Activate outside polling through
  `RegistryQuery`, the shared CLI transport and decoded snapshot owner.

## Desktop integration after upstream #1447 — October 5

Merge `c4373492c` integrated base `1f62e836d`, including #1447's
cached-workspace rendering changes.
`PodiumModel` restores the saved workspace and retains last-good planning on read
failure; its planning and Session refresh loops run independently. `WavesView`
now creates its window model lazily. `WaveDetailPane` separately polls status
and renders `reading.plan(cached: plan)`. None of these paths ensures a Project.

Keep activation at the explicit Wave-opening/retry boundary through the shared
transport, outside model construction, cache restoration and both periodic read
owners. Preserve the existing saved-plan presentation and independent Session
refresh while ensure runs. Headless Desktop acceptance must include reopening
from saved state with failed or delayed ensure, and switching Waves before that
response returns. #1447's returning-launch measurements do not prove Project
readiness or this activation path. This refines implementation step 6 without
changing Jack Heart's accepted opening behavior.

## Chosen operation and authoritative state

Shared local Wave configuration (selected implementation layout):

```yaml
# <Home>/waves/<WaveId>/config.yaml
pm:
  linear_project: <stable Linear Project UUID>
```

The configuration owns which Project is current. Linear owns that Project's
status, content and membership. SQLite retains its existing synced read model,
not an independently writable current-Project pointer. A transition receipt may
record intended changes but never overrides the configured selection. Preserve
other configuration keys and authored bytes when updating this field.

`lf wave ensure <wave> --json` is the explicit activation operation shared by
Desktop and agents. It takes no candidate-selection argument. To use an existing
Project, configure its exact ID. Reads never write configuration or provision
Projects. No matching by name, ranking by status, or candidate discovery belongs
in this API.

1. Acquire the per-Wave planning lock shared with plan editing and reset. Resolve
   the configured Project, repository/Team/Initiative and pending transition.
   Do not contact unrelated Waves or launch a Session.
2. With a configured ID, fetch that exact Project and confirm ownership. Reuse
   an In Progress Project; activate the configured Backlog/Planned Project by a
   status-only write. Names and missing Flow do not affect selection. A failed
   lookup, archive, deletion, terminal state or changed ownership is actionable
   evidence about that binding, never permission to select or create another.
3. With no binding, resume an unfinished creation by its recorded ID, or reserve
   one new UUID and create an ordinary Project named for the Wave, with no default
   Flow. Attach and activate it, then save its ID to the Wave config. A pending
   explicit reset retains its own recovery; opening never starts a competing
   creation. No search for adoptable Projects precedes ordinary creation.
4. Confirm provider facts and atomically save the reference while preserving
   unrelated config edits. If a response or config write fails, retry the same
   reserved ID. Never overwrite a different ID written in the meantime. Read
   back configuration and provider outcome before settling the operation.

The configured ID is the only current-Project selector. Other provider Projects
may exist; their names and statuses cannot change that selection or block opening
this Project. The API does not promise exactly one In Progress Project throughout
Linear; it maintains one configured current Project for the Wave. Terminal,
archived or inaccessible configured Projects report their condition; opening
never silently reopens or replaces them.

Existing Waves need their known Project UUIDs explicitly seeded in configuration
as part of rollout, preserving existing work. That setup is separate from the
runtime API; do not ship a heuristic matching layer as a migration convenience.
Creation requires successful connection/access checks. A provider outage with
no configured ID must not be interpreted as permission to create elsewhere.

Ensure and observational readers resolve the same configured ID. During a reset,
the predecessor remains selected until the explicit configuration switch; afterward
the successor is selected and unfinished predecessor work remains visible. Opening
does not move Tasks, complete Projects or switch the reset's config reference.
Status reports pending recovery separately from current selection and marks
stale/unavailable provider evidence truthfully.

Inspection of `ops/pm.rs::resolve_context` and `planning.rs` found configured
Linear planning, not a local-only Project writer. Missing connection reports the
existing connection action; outages never invent local Projects. Configuration
selects identity; it is not a second planning store.

## Persist only the missing recovery evidence

Add one narrow Project transition receipt in the existing SQLite owner, using
one migration draft for this Task. Receipt fields: Wave ID, reserved successor
provider ID, optional predecessor provider ID, optional explicit reset name,
creation timestamp and settlement timestamp. A unique unfinished transition per
Wave serializes admission alongside the Wave lock. For reset, persist the exact
selected predecessor/successor before effects; for initial ensure, predecessor
is null. Existing Project facts remain in their existing tables. Do not copy
Tasks, KRs, names, provider status or execution cursors into this receipt.

This is operation recovery evidence, not a Chapter object or second plan. Fresh
provider observations determine the next missing effect; a phase counter must
not assert an effect happened. Readback settles the receipt only when attachment,
the configuration reference, and any predecessor completion are confirmed. Preserve it
across crash, response loss and process replacement. Query it by Wave and, for
reset re-entry, its recorded target IDs; reset names are descriptive metadata,
not Project lookup keys. Do not select “latest” by timestamp.
Retain settled receipts as operation history without giving them current-plan
authority. An unresolved older transition cannot be overwritten by another reset.

The supported concurrency boundary is the configured owning Home, consistent
with Jack's one-client decision. Competing Desktop/CLI processes share the lock
and reservation. External Linear edits are not lockable; recheck ownership and
surface conflict. Do not claim a distributed transaction or invent a fleet lock.
Retain explicitly selected reset successor IDs; allocate new IDs independently
of display names. Remove name-derived successor UUID generation.

## Optional coordinated reset

Keep explicit `lf repo new-chapter <name> --dry-run --json` and apply as the
coordinated operation. Tools and their operating skills coordinate participating
Waves across the repository and through its hierarchy, with a unified chapter
name and metadata. Approximate synchronization permits recoverable partial
progress; it does not impose chapter membership on ordinary Project access.
Every completed rotation selects one current Project per Wave in configuration. Desktop's “Realign Projects…” previews the same result
and applies only after an explicit action. No reset occurs on Wave opening.
Expose affected Wave, predecessor/successor and Task dispositions in that preview;
show unavailable Waves and unresolved work before applying. Require supplied KRs
for every participating successor before the first chapter creation mutation.
Ordinary ensure has no KR requirement.

Replace shared-predecessor-name inference with independent per-Wave selection
plus the exact transition receipt. The configured Project identifies each
predecessor; different predecessor names are normal.
Reset accepts exact successor IDs or reserves new ones. Do not locate targets
by chapter name. Existing explicit target Projects retain identity and content;
apply only the authored next-plan changes supplied to the operation. Copy an
optional Flow only when present. Install the supplied new KRs and any supplied
metric targets; preserve predecessor KRs, targets and outcome evidence. Never
create an empty-KR chapter Project and promise to plan it afterward. A reset can
start with no prior chapter or no prior Project.

Retain complete preflight before the first coordinated provider write. Persist
all selected transition identities before apply. Acquire participating Wave locks
in stable ID order to avoid deadlock; ordinary ensure only acquires its own lock.
Re-entry recovers each pair, including successor activated, Task partly moved,
predecessor completion response lost, or earlier Waves already settled. A later
Wave failure leaves earlier progress intact and reports where retry resumes.
Pending global progress does not add a prerequisite to an unrelated Wave.

Keep fresh Task classification, missing-membership exact lookup, Task-start
reservation fencing and positive cancellation confirmation. Retain accepted full provider readbacks and atomic projection for Task transfers;
the direct `move_chapter_task` writer is deleted.
Started/claimed/authored/published work moves with its Task ID,
checkout, PR, managed Flow and independent Session/Exec association unchanged.
Unknown evidence remains unresolved. Remove automatic backlog expiration on
chapter rotation. Started work carries forward; unreviewed backlog remains
visible under its original Project until explicitly carried, revised or retired.
Even proven untouched Tasks do not expire merely because the chapter changes.
This preservation rule supersedes the earlier draft's automatic expiration.
Expose unresolved backlog in the planning follow-up without making an exhaustive
review or every future Task a prerequisite for creating the successor.
After successor activation and required Task dispositions are confirmed, update
the config reference from the recorded predecessor to the successor, then
complete the predecessor. Persist both IDs before effects; retry accepts config
matching either endpoint and reconciles the missing effects. A different config
ID is an intervening decision, not permission to overwrite it. Config and provider
changes are not one transaction: prove crashes on both sides of the switch.
Complete predecessor after transfer, never merely because successor activation
succeeded. No global rollback, Session restart, Task completion or inferred KR win.

## Chapter review, planning and Task admission

Accepted October 4 boundary from Jack Heart:

1. **Review and plan:** a skill reviews the previous chapter's outcomes and
   unfinished work, then formulates the next chapter's per-Wave KRs. It may
   produce proposed metric targets and Task candidates alongside them. Reuse and
   reconcile the existing repository/Wave `review-chapter` and `start-chapter`
   skills rather than building a second planning framework. Final skill/Flow
   composition and names remain implementation choices.
2. **Create chapter Projects:** the synchronous operation takes each selected
   Wave's chapter key, name and nonempty authored KRs, plus optional targets and
   Flow. It creates/configures successors and preserves ongoing work. It does
   not generate KRs, await an agent, or require newly planned Tasks. Validate KRs
   before provider writes; the planning skill owns their substantive quality.
   Confirm those KRs in provider content before switching the Wave's reference.
3. **Add Tasks:** a separate step reviews/adopts Task candidates into the exact
   created Projects. It can run across all participating Waves immediately after
   creation or incrementally later. Re-enter through the existing Task creation
   owner without duplicating already admitted candidates. Its failure leaves the
   chapter Projects and KRs usable; it does not roll back Project creation.

Candidate notes are authored planning output, not durable Tasks or another
planning database. They must survive between these steps and remain available
when Task admission fails. Use the existing planning artifact path; retain links
to destination Project IDs after creation. The exact output format remains to
be specified with the skills. No complete future Task inventory is required.

Preserving existing work belongs to rotation, not the optional creation of new
Tasks. Review and planning account for old KRs and backlog; neither an empty new
Project nor wholesale old-Task cancellation establishes a successful transition.
Initial KRs are required for chapter creation but can evolve through ordinary
Project edits afterward. Ordinary non-chapter Project ensure stays independent
of this review/planning sequence and may create a Project without KRs.

## Chapter facilities and history

October 4 direction from Jack Heart: retain higher-level facilities to create a
Project for a new chapter for a particular Wave. Jack also raised cross-Wave
historical chapter inspection as a capability to design. These facilities compose
the explicit Project operations; they do not add discovery heuristics to ensure.

Proposed interface shape (command spelling is not settled):

- Wave-scoped chapter rotation accepts a Wave, explicit chapter key and authored
  KRs, creates
  a successor or uses an explicitly supplied Project ID, preserves ongoing work,
  and switches that Wave's configured reference through the reset path above.
- Repository-wide rotation invokes that same operation for participating Waves
  with one chapter key. The current repository reset API remains the integration
  point; it must not grow a separate implementation of Wave rotation.
- Read-only chapter inspection takes the chapter key and returns linked Projects
  across Waves, including completed/archived history, their Wave identity,
  provider ID, name and observed status. It reports unavailable history as such.

Proposed data: optional chapter metadata on each participating Project containing
an immutable shared chapter key and a display name. The key is chosen once for a
coordinated chapter and passed explicitly to each Wave operation; Project names
remain independent and may change. Ordinary Projects omit the metadata. Each
historical Project retains its own membership when the Wave config advances.
No Chapter table or independent chapter lifecycle is needed for this lookup.

The metadata's exact provider representation and projection into the existing
SQLite planning model remain to be designed. Prefer existing Project content
metadata if sufficient; do not assume Linear provides a custom field. History
must retain exact known Project IDs when current Initiative membership omits old
Projects. Chapter inspection queries explicit metadata, never title parsing or
current Wave config alone. Do not infer historical membership from similar names;
any backfill uses explicitly established associations.

Cross-Wave historical inspection is a proposed extension, not yet a required new
CLI surface. Before including it in delivery, settle its representation and prove
that renamed and completed Projects remain discoverable after later rotations.
Wave-scoped chapter creation is part of the requested higher-level design.

## Implementation order

One delivery, sequenced by dependency. The detailed contracts above and acceptance
cases below own requirements; this list identifies each next cut:

1. Cancellation-safe acceptance, cold ownership re-read, full reteam readbacks
   and authorized Team acceptance are implemented and pass the focused operation
   checks below. Preserve them across the remaining cut. Registration
   carries its Wave guard through commit and selects accepted issue facts. Created-successor
   recovery remains with steps 3–5, not a repair to the old name selector.
2. Preserve transactional registration and rotation checkout exclusion. The new
   operation regressions cover newer accepted facts, stale-owner refusal, both
   start/rotation orderings and failed-reset retry. Final before/after-configuration-
   switch proofs and created-successor recovery depend on steps 3–5.
3. Add the shared Wave-ID configuration reader/writer and explicit binding setup,
   then replace every current-Project selector, including Task routing and
   CLI/status/DTO consumers. Preserve the designated Intelligence Project.
4. Add transition persistence and ensure, exercising concurrency and uncertain
   responses through operations with a stateful fake provider. Test this Task's
   single migration draft from the released frontier.
5. Replace rotation through Wave-scoped KR-first creation and repository
   composition, retaining preservation coverage with the new recovery semantics.
6. Add Desktop activation and explicit reset preview/apply/retry per **Outcome and
   demo**. Rename ordinary Chapter labels to Project; keep metrics with Project.
   Change Rust/Swift DTO fields and fixtures together, without defaults.
7. Update `docs/lf.md`, command reference, planning architecture, repo guide and
   builtin `review-chapter`, `wave/review-chapter`, `start-chapter`,
   `wave/start-chapter`, `repo/session`, `wave/session` at their owners. Implement
   the separate KR planning → chapter creation → Task admission sequence, retaining
   candidate output. Upstream #1446 composes `repo/operate` and `wave/operate`
   into their ongoing conversations; preserve those procedures and started-Task
   follow-through. Explicit begin-work/activation may ensure; fresh status reads
   in those procedures remain observational. Reconcile LOO-367's call site
   without its lifecycle changes.

## Delete — do not maintain

Apply the remaining cuts with their replacement consumers under the selected
shared local configuration owner. The empty-Flow and name cuts are implemented locally; shared selection and
rotation remain. Keep this one delivery boundary.

- `ops/chapter.rs::select_current`: replace status selection with the shared
  configured-ID reader. `sync_projects` and `record_project` are deleted.
- `plan_rotation`'s `predecessor_names` and target-name lookup, plus `successor_id`:
  replace inferred predecessors and name-derived UUIDs with exact recorded IDs.
- Deleted: `canonical_project_name`, Task-resolution projection, sync/reteam
  prefix rewrites, the unused `rename_project` API and reteam’s `target_name`
  field/rename display. Provider IDs establish ownership.
- Deleted: rotation's durable-only `record_project`/`move_chapter_task` writers.
  Accepted readbacks preserve exact transfer identity; full recovery still needs
  the selection/transition replacement.
- Deleted: `update_task_plan`, generic `update_task`, their async wrappers,
  `TASK_UPDATE`, and restart's captured Project/Task plan writes. Preservation
  assertions use accepted ingestion; placement fixtures seed their own rows.
  Generic runtime `update_project` and its SQL are also deleted. Fixture setup
  remains local; behavioral updates use accepted observations.
- Deleted: `project_params` and `task_params`, allocating parameter builders left
  behind by the generic update APIs; each insertion binds values beside its SQL.
- Deleted: SQLite's separate `insert_task_with_worktree` transaction; both public
  registration APIs use one insertion path, with an optional initialization event.
- Deleted: `create_task_work`'s duplicate Task lookup/insert and fallback identity.
  Registration inserts the Task once; `inherit_task_placement` only inherits its
  Project placement in the same transaction.
- Deleted: `ops/pm.rs::store_pm_snapshot`; held-lock refresh accepts the already
  registered Wave directly, without rediscovering its identity after acquisition.
- Deleted: checkout admission's Task-population scan. Shared ancestor locks cover
  future Task roots without a population owner or discovery retry.
- Deleted: `sessions::replace_input_in`'s generic `cwd` update. Input replacement
  retains the stored workspace; explicit primary workspace admission remains.
- Deleted: rotation's automatic backlog cancellation, `retire_chapter_backlog`
  and its exclusive retirement test. Unreviewed Tasks retain their Project;
  explicit Task cancellation keeps its separate owner and confirmation.
- Deleted: `move_project_to_team`, its duplicate GraphQL mutation and exclusive
  wiring test. Expansion and narrowing both use `set_project_teams`; the operation
  test retains resulting Team membership, names and issue identifiers.
- Deleted: `rebind_task_issue_identifier`, both callers and its exclusive test.
  Full accepted issue readbacks preserve UUID, Task/PR identity and revisions;
  explicit Team reconciliation supplies authorized relationship evidence.
- Replace obsolete assertions in `ops/chapter_tests.rs`, including
  `zero_current_without_shared_predecessor_evidence_stays_unresolved`,
  `lost_creation_has_one_identity_on_every_home` and
  `only_proven_untouched_backlog_expires`, with configured-ID recovery and backlog
  preservation proofs. Keep shared provider fixtures and identity-preservation
  cases; they also exercise surviving behavior.

Move reusable functions rather than copy them. Retain migration-marked conversion
only for released-data preservation, removing its Flow refusal. Add no candidate
selection, Project variants, chapter-required adapters, DTO defaults or generic
recovery framework.

## Acceptance at gate

New test names below are implementation targets, not existing passing checks.
Use the repository's isolated fixture environment and compiled test CLI; prevent
native providers and configured accounts from being launched by synthetic tests.

- `cargo test -p loopflow --lib project_ensure`: no Project → one Started record;
  repeat/two processes → identical UUID and one provider Project; ordinary
  configured Backlog and current no-Flow Projects preserve all bytes/IDs;
  Task resolution, sync preview/apply and reteam retain ordinary provider names;
  configured identity is independent of other Project names/statuses; absent
  configuration creates and records one ID without candidate discovery; failed
  access produces no provider writes; interrupted create/attach/activation
  resumes exact UUID; archived pending identity is not recreated. Failed config
  writes retry the same identity; concurrent config edits survive. Assert outcomes,
  not mock call wiring.
- `cargo test -p loopflow --lib chapter`: different ordinary names, missing Flow,
  no previous chapter, response loss at every mutation, unrelated Wave outage,
  and externally changed configured/recorded successor identity. Snapshot Task, PR, checkout, Session and Flow
  identity before/after; retain start-versus-retire and unknown-evidence cases.
  Missing/empty KRs reject chapter creation before provider writes; supplied KRs
  survive retry without duplication or lost content. Chapter creation succeeds
  with no new Task candidates. Ordinary ensure still succeeds without KRs.
  Unreviewed backlog remains visible and unretired after rotation; predecessor
  KRs and outcome evidence remain intact. Failure in later Task admission does
  not undo Project creation or its configured reference.
- Skill/Flow acceptance demonstrates previous-chapter review producing next KRs
  and retained candidate notes, chapter creation completing before Task admission,
  and global follow-up resuming without duplicate Tasks. Verify authored order
  and real operation outcomes, not just matching phrases in prompt files.
- Add `rust/loopflow/tests/project_readiness.rs` and run
  `cargo test -p loopflow --test project_readiness`: CLI ensure → fake Linear
  state → persisted planning → Wave status/roadmap and JSON consumed by Desktop.
  Repeated status/roadmap must leave provider Project count/status and transition
  admission unchanged. Failed reset on Wave B must not block ensure/use of Wave A.
  At every reset interruption, ensure, status and roadmap select the configured
  ID, retain predecessor Task visibility and leave reset effects untouched on
  opening. Cover crash before/after config switch and provider completion; stale
  checkout configuration must not reverse a later binding or resume an old reset.
  Use two linked checkouts sharing one fixture Home: after creation in A and
  receipt settlement, B reuses the exact ID; after reset in A, stale B reads the
  successor without Git sync. Both absent and stale checkout files are ignored.
  A malformed shared file causes no provider writes; read-only access creates no
  file or directory. Preserve unrelated file bytes and test interrupted replacement
  and a Wave rename against the same stable-ID file.
- `scripts/test_desktop.sh -Xswiftc -gnone`: headless activation/reopening,
  selection race, retry/error, Project labels, independent primary conversation,
  reset preview/apply and JSON fixtures. `uv run python
  scripts/check_swift_multiplatform_boundaries.py` retains shared transport boundary.
  Use app/view builds/tests; no screenshot, display or permission dialog required.
- Gate runs affected migration/architecture checks, formatting and all-target
  Clippy once under TESTING.md. Implement only builds and runs its focused cases.
- Jack Heart's October 5 comment `74eee6f5-055f-4be1-914f-9272d63cbf47`
  prohibits three installation proofs on this host:
  `global_commands::installation_uses_candidate_authority_from_any_checkout`,
  `global_commands::installation_reaches_candidate_verdict_with_an_unreadable_task_registry`
  and `exec_ownership_tests::early_observation_records_preflight_and_screenshot_child_ancestry`.
  Installation preflight/promotion resolves the OS account Home through `getpwuid`;
  changing HOME/LF_HOME still copies the live database. Integrated PR #1444
  (`022a248df`, included by merge `596572efc`) marks these tests ignored in the
  regular suite and registers them in `scripts/test_task_installation.py` under
  a disposable OS account. Gate/CI retains that harness; the source change proves
  isolation routing, not successful execution. None ran in this reconciliation.
- Configured acceptance remains required after implementation: use explicitly
  designated fixture Waves for absent, ordinary Backlog/current and reopening
  scenarios, record provider UUID/status plus Task identities, and demonstrate
  useful Wave A operation while another Wave/reset is unavailable. Configured
  proof has not run. If fixture destinations or write authorization are
  unavailable then, report that exact acceptance gap; do not reset production
  Waves or count simulated success as configured proof.

Done means the opening/CLI experience works and repeated calls preserve identity,
ordinary names and empty Flow are usable, reads remain observational, and explicit
reset recovers without sacrificing started work. Full Task admission/completion,
new primary Session behavior, credential repair, release plumbing and a generic
multi-product platform are excluded.

## Review constraints

Prior compression rationale is retained at
`f2b127d87bc0bb99654ba66118fb91a77099b73f:scratch/keep-every-wave-ready-for.md`
under this heading. Shared `associate_project` rollback, independent entity ages,
`run_planning_write` guard lifetime, stable-ID lock order and exact Team readbacks
remain required; full-refresh freshness differs from partial acceptance.

Shared acquisition, first-error collection, membership and migration-slice
boundaries remain required. Prior compression detail survives at
`a7789f1b88beb125110b36f511510701c463fd1d:scratch/keep-every-wave-ready-for.md`.
CI repair retains outer admission without reacquiring it in reservation.

The recorded nextest pass also reports a leaky projectless-Task case. Its cause
is unknown; gate retains output-handle investigation, not an assumed harmless leak.
Cached-name conversion and shared selection must land together. The approved
historical name-only exception is implemented; the old selector remains to replace.

Reserve identity before provider effects so a timeout cannot create another UUID.
Use status-only writes and byte-preservation tests to protect authored content.
Review rejected bootstrap chapters, creation during status reads and name-derived
permanent Project IDs. Keep recovery receipts confined to mutation recovery;
provider status/content and configured selection retain their respective owners.

#1451's pending-version behavior retains the name-cutover fixture's same-batch
boundary; no release-tree gate ran here. Merge `596572efc` also includes the
v0.13.4 version preparation at `99aa64dbd`; this is source integration, not
publication or installation evidence. Release's child memory retains manual
v0.13.3 verification and unresolved unattended settlements. Its operation-entry
recovery lesson still applies to ensure and rotation: helper/store proofs do not
establish recovery across the configuration switch.

Checkout admission now lives in `store/sqlite/admission.rs`, shared by registration,
Sessions, Flows and CI repair. Path modes accumulate directly in the ordered map;
there is no intermediate path list. The registration retry test also covers the
former standalone earlier-conversation case, preserving all assertions for both
APIs. Review retained canonical path order, ancestor sharing and guard lifetime
through commit. The new rotation lock group retains checkout exclusion through
queued projection too; shared selection and readiness remain unfinished.
Earlier checks and the failed Home-wide fence remain in the history above.

Reteam's retained compression and earlier check evidence remain at
`3555375af:scratch/keep-every-wave-ready-for.md`; its full preflight, exact readbacks
and cancellation-safe acceptance remain required.

Checks: reconciliation `git diff --check` passed; prior results retained without rerun: `cargo test -p loopflow --lib --no-run --jobs 4`, `cargo fmt`, `cargo clippy --all-targets --jobs 4 -- -D warnings` passed; isolated `scripts/test_network.py <lib-test> <filter> --test-threads=1` passed 13 tests across `registration`, `missing_root_exclusion`, `cancelled_projection`, `rotation_excludes_checkout_starts`; gate retains full affected suites, CI repair and configured acceptance. Earlier checks remain at `7e7204ddad:scratch/keep-every-wave-ready-for.md`.
