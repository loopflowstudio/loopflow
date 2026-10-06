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

Optional Flow, the cached-name cutover and shared binding setup are implemented
locally. `lf wave bind-project <wave> <uuid> --json` validates exact ownership under
the Wave lock and writes the stable-ID file atomically, preserving unrelated YAML
bytes. Repeated binding is idempotent; replacing a different binding remains with
rotation. The command preserves status, including Backlog without Flow. Accepted
Project ingestion now returns its committed body; a delayed Backlog response cannot
bind a newer completed Project. File reads create no directories. Leading-comment
preservation requires editing the YAML file rather than only its document.

Operation routing, metrics and Project editing now select the shared ID through
`project::current_project`/`select_project`. Status/roadmap retain all Project/Task
planning; Rust/Swift summaries carry required `current` selection rather than
Desktop inferring it from Started status. Retained Task navigation uses the latest
Wave even during a selection switch. SQLite admission now validates the binding. Exact-ID ensure and creation recovery are
implemented below; rotation still uses name selectors and Desktop activation is unfinished;
Intelligence has not been bound or activated on the installed Home.

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
ensure; its implemented CLI spelling is `lf wave bind-project <wave> <uuid> --json`. It must validate the
exact Project's ownership under the same Wave lock and preserve unrelated file
bytes. Ordinary ensure must not infer this binding from candidate discovery.

Source inspection confirms `migration_adoption_accepts_a_started_project_without_flow`
uses Started status and disables promotion. It proves neither Backlog activation
nor the configured Intelligence repair. Existing migration adoption can promote
only from retained migration evidence; it is not a general bootstrap API.
The empty-Flow refusal is removed; explicit binding and status-only ensure exist
in source. No installed repair is established, and a branch binary must not write
the installed Home. Delivery must report the actual supported binding/activation
commands once implemented and available in a published installation.

The Backlog/empty-Flow operation fixture now binds and activates the exact UUID,
preserving provider content/KRs and issue bodies on repeat without creation. It
does not seed local Task/PR/Session/Flow execution; that identity proof remains
with the CLI/rotation acceptance. Configured acceptance
for Intelligence has this explicit destination and repair authority; other fixture
Waves still need designated destinations. No global reset is authorized.

## Cached-name cutover — approved and implemented October 5

Jack Heart authorized one historical name-only correction in comments
`5419b87c-bfec-4f42-8914-021483249895` and
`fb2d9c94-ad0c-4762-8658-213734ddc5c2`. The existing `project_readiness` draft
archives original bodies/times and grants only pre-cut rows one name/slug correction
at equal revision. Older/rejected writes cannot spend it; all non-name conflicts
and subsequent conflicts stay strict. Acceptance and projection share a transaction.

Names remain verbatim through lookup, sync and reteam. Snapshot replay, generic
planning writers and identifier-only transfer writers are deleted. Retain original
body/time, rollback, older-acquisition and strict-repeat proofs. Detailed implementation
and exact evidence remain at
`57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.
Local synthetic passes establish neither installed conversion nor configured readiness.

## Remaining coherent cut

Registration and rotation membership work, exact-ID readers and SQLite admission
are implemented. Both registration APIs share accepted-fact insertion and retain
Wave/checkout guards through commit. Already-started Tasks can continue in a
predecessor. Retained evidence, malformed-body failures, writer inventory and
identity-preservation details remain below and at
`1825a5a45c743833f625e8c8144949ed139b97f9:scratch/keep-every-wave-ready-for.md`.

October 5: step 4 now adds `project_transitions` to the existing `project_readiness`
draft and exposes `lf wave ensure <wave> --json`. One unfinished receipt per Wave
reserves a random UUID before provider creation. Queued receipt writes retain the
Wave guard. Ensure checks exact destination access, recovers an unattached reserved
Project, activates Backlog/Planned with status-only writes, accepts provider facts,
then writes/reads the shared binding and settles the receipt. Ordinary ensure has
no KR or Flow prerequisite and no candidate scan. It leaves pending reset recovery
to rotation. Explicit binding cannot strand another pending reservation.

Six stateful operation tests pass (the `project_ensure` filter also includes one
migration test): concurrent callers; response loss after each
of create/attach/activation; failed binding replacement and post-binding settlement;
archived reservation and unavailable reads; existing Backlog with empty Flow,
authored content and Tasks; delayed Backlog after accepted completion. Review
caught activation preceding accepted-fact ingestion: ensure now accepts the read
before deciding whether to activate, preserving newer completed history. A separate
released-frontier migration test checks pending uniqueness and settled history.
These are synthetic operation proofs, not configured Intelligence acceptance.
Concurrency uses two futures in one process. The post-binding recovery case seeds
the complete binding after a failed write; it does not interrupt a real process
between binding and settlement. Cross-process/cross-checkout and crash proofs remain
with gate. Existing check results cover the local `accept_project` consolidation.

Step 5 remains next, with the recovery correction below. Rotation still uses
names/status and neither consumes transitions nor switches the binding. The
branch is not releasable. Desktop, skills and configured acceptance remain open.

October 5 realignment confirms the remaining preflight gap in `rotate`:
Task evidence is collected across Waves, but exact successor ownership is checked
inside each `apply_rotation`, after earlier Waves may have mutated. The same
loop calls Wave-wide `adopt_legacy_projects(..., true)` before applying its pair.
The replacement must include any required legacy conversion in whole-input
preflight and reservation ordering; it must not activate unrelated legacy
Projects outside the selected pair. Retain released-data conversion in its
existing owner. A later Wave with invalid KRs, inaccessible/foreign destination
or invalid conversion evidence must cause zero provider writes across the input.
This is source inspection, not an executed failure proof.

### Rotation recovery correction — October 5

Source inspection found two missing inputs in the earlier receipt contract.
These are architectural counterexamples, not executed interruption tests. The
superseded contract remains at
`57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.

1. Wave A settles, then Wave B fails. Retrying only `new-chapter <name>` cannot
   identify A's settled destination without matching names, consulting a receipt
   as current selection, or mistaking A's new binding for another predecessor.
   `pending_project_transition` deliberately omits settled receipts. Preserve
   exact destinations in the caller's retained planning artifact; query receipts
   by `(Wave ID, successor ID)`, including settled history. An intervening binding
   remains a conflict; an old plan must never rotate the Wave backward.
2. Task X transfers, the binding switches, and predecessor completion fails.
   During the interruption, backlog Task Y starts in the predecessor and an
   external edit returns X there. Accepted refresh overwrites `pm_items` and
   projects both current memberships into `tasks`; it retains no transition
   membership. Reclassifying sweeps Y into the successor; ignoring the predecessor
   loses X's required reconciliation. The same problem exists for provider Tasks
   without a local Task row. The receipt must retain the exact selected issue IDs,
   independently of current membership. No Task bodies or execution ownership are
   copied. An external move of a selected issue after the switch is reported as a
   conflict, never overwritten merely because its ID appears in the receipt.

Implementation choice, requiring no new product decision: KR planning emits a
retained JSON plan containing the chapter name and per-Wave stable identity,
exact successor UUID, explicit create/existing intent and Project content. Allocate
new UUIDs once while authoring that file. Both Wave and repository operations take
that same input; the repository operation composes the Wave operation. CLI uses
`lf repo new-chapter <name> --plan <path> --dry-run --json`, then the same file
without `--dry-run`. The name must agree with the file and never selects a Project.
Existing destinations must exist; creation reservations cannot substitute for
missing existing Projects. The artifact is command input, not current selection.
Retain it alongside Task candidates through partial failure and Task admission.

Extend this Task's existing migration draft with transition-owned issue-ID
membership. Before each transfer, persist its selected ID under the retained Wave
and checkout guards. Before the binding switch, recovery reclassifies remaining
predecessor work and retains earlier selections, including uncertain transfers.
After the switch, recovery uses only that set; new historical starts stay put.
Reservation records intent, never successful provider effects. Exact readbacks
still own transfer confirmation, and Task/PR/Session/Flow rows keep their owners.

Preflight the whole input, nonempty KRs, destination access and Task evidence before
provider writes; persist every participating reservation before applying any Wave.
Retry validates the input against the reserved pair and selected binding. Settled
receipts establish only that exact operation's history. They cannot choose the
current Project or authorize reversing a later binding.

Required operation proofs: partial repository settlement then retry with zero
extra Projects; same display name used for two distinct explicit plans; selected
issue moved externally after the switch while another historical Task starts;
pre-switch response loss followed by new work; and caller cancellation around
membership persistence. Retain the existing created-successor, backlog, checkout
exclusion and execution-identity proofs. None of these new proofs has run.
The proposed `--plan` flag, Wave-scoped operation and settled-transition lookup
are not implemented; the CLI still accepts only name, dry-run and JSON. Existing
start-chapter skills still prescribe empty-KR creation, required Flow and backlog
cancellation. Their coordinated replacement remains step 7, not completed behavior.

### Accepted planning must own durable projection — October 5

Collection now preserves issue-reported ownership, including moved and detached
issues. Its loopback regression failed before removing the listing-Project overwrite.
Atomic projection must consume those reported facts, never reattach them to the
queried Project. Rotation/reteam accept full readbacks; the identifier-only writers
are deleted. Exact collection evidence and the old ownership counterexample remain
at `57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.
These synthetic passes do not establish configured recovery.

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

### Created-successor recovery — retained failing evidence

`rotation_recovers_a_created_successor_after_confirmed_transfer` loses the
connection after transfer readback. Accepted planning, Task/PR/checkout and Flow
survive, but retry fails on `A — next, A — previous`. The existing-successor case
completes. Different predecessor names also broke the two-Wave backlog fixture;
its narrowed matching-name pass is not differing-name coverage. Details remain at
`57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.

Exact transitions replace that selector; never restore name stripping or extend
the obsolete inference. The October 5 recovery correction above owns the missing
inputs. Ensure's proofs do not establish rotation recovery.

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
  boundary. Registration now selects by shared binding. New-issue
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

Detailed Session/Flow admission evidence remains at
`c31279995a4ea0eec09c053e39c2f71a81d26034:scratch/keep-every-wave-ready-for.md`.
Input replacement retains stored workspace and bound Task roots; CI repair retains
outer admission without reacquisition. Task-bound Flow cwd derives from the Task.
Managed-Flow setup does not itself mark Started; worker claims already acquire
checkout exclusion. No new setup lock was added. Rotation proofs still require the
configuration switch, and CI repair's operation-entry proof remains with gate.

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
switch, retry checks the persisted selected issue IDs and predecessor completion
without sweeping new historical bindings into the successor. External membership
changes of selected issues remain conflicts, not implicit permission to move them. Pending receipts alone never
deny starts. Prove both start/rotation orderings and response loss on both sides
of this boundary through operation entry points, retaining actual failures.
Release's recovery findings reinforce this requirement; its publication and
installation evidence supplies no Project-readiness acceptance.

Historical binding, unbound checkout transfer and both registration APIs retain
focused store proofs. Their exact names and boundaries remain at
`986be7988:scratch/keep-every-wave-ready-for.md` under “Preservation boundary”.
Rotation adds both start orderings and failed-reset retry. Configuration-switch
and created-successor recovery remain unproved; explicit selection is unchanged.

## Outcome and demo

Every Wave has one explicitly configured current Project, normally In Progress.
Opening a Wave in Desktop ensures that Project independently of any chapter. Existing
names, content, Tasks, checkout, PR, Session and Flow identities survive. CLI and
agents use the implemented `lf wave ensure <wave> --json`. Status,
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

`work/wave/config.rs` retains authored policy; `project_binding` owns only shared
Project selection. `ProjectContent` keeps empty-string Flow; LOO-367 owns launch
validation. `checked_projects_with_store` retains exact missing-membership lookup
and ownership checks. Linear creation accepts a caller UUID, but attachment is
separate; `find_project` distinguishes archived from absent. Duplicate-ID and
attachment behavior still need configured proof. `pm::lock_wave_planning` supplies the
shared per-Wave OS lock with a 30-second bound.

Desktop activates through `RegistryQuery` and the shared CLI transport outside
30-second polling. Exact schema references and earlier implementation anchors:
`57ac8b09fbbff3f3b7c82c00ecf9032f0cb792b1:scratch/keep-every-wave-ready-for.md`.

## Desktop integration after upstream #1447 — October 5

Merge `c4373492c` integrated #1447's cached-workspace rendering changes.
`PodiumModel` restores the saved workspace and retains last-good planning on read
failure; its planning and Session refresh loops run independently. `WavesView`
now creates its window model lazily. `WaveDetailPane` separately polls status
and renders `reading.plan(cached: plan)`. None of these paths ensures a Project.

Keep activation at the explicit Wave-opening/retry boundary through the shared
transport, outside model construction, cache restoration and both periodic read
owners. Preserve the existing saved-plan presentation and independent Session
refresh while ensure runs. Headless Desktop acceptance must include reopening
from saved state with failed or delayed ensure, and switching Waves before that
response returns. #1454's benchmark integration changes no activation. First-frame timing stays
separate from ensure completion; no new launch benchmark is required. Earlier
measurement limits remain at `1e5086aa3:scratch/keep-every-wave-ready-for.md`.

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
Projects.
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

The configured ID alone selects the current Project; other Projects cannot block
opening it. This does not promise one In Progress Project throughout Linear.
Terminal, archived or inaccessible bindings report their condition without replacement.

Rollout explicitly seeds known Project UUIDs, preserving existing work without
heuristic matching. Creation requires successful connection/access checks;
an outage never authorizes creation elsewhere.

Readers share ensure's configured ID: predecessor before the reset switch,
successor afterward, with unfinished predecessor work still visible. Opening never
moves Tasks, completes Projects or switches a reset binding. Status separates
pending recovery from selection and marks stale/unavailable evidence truthfully.

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
is null. Existing Project facts remain in their existing tables. Retain selected issue IDs as transition-owned membership, as specified in the
October 5 recovery correction. Do not copy Task bodies, KRs, provider status or
execution cursors into this receipt.

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

Use `lf repo new-chapter <name> --plan <path> --dry-run --json` and apply as the
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
Reset consumes exact successor IDs from the retained plan and reserves them
before effects, with explicit create/existing intent. Do not locate targets
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
3. Shared binding, routing, Project editing, Rust/Swift readers and SQLite admission
   now select the exact configured ID. New registration and unstarted managed work
   require Started status; already-started work continues in a predecessor. Focused
   proofs retain Task/PR identity, rollback and queued guard lifetime. Store fixtures
   now configure IDs; operation/CLI fixtures remain with the ensure/rotation cut.
   Preserve the designated Intelligence Project.
4. Transition persistence and exact-ID ensure are implemented with stateful
   operation recovery coverage. Retain the released-frontier migration test;
   cross-checkout CLI proof remains with gate's project-readiness acceptance.
5. Implement the October 5 recovery correction: retained exact-ID input,
   transition-owned selected issue IDs, Wave-scoped KR-first rotation and
   repository composition; prove both sides of the binding switch.
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
shared local configuration owner. Empty-Flow, names and reader selection are implemented
locally; rotation remains. Keep this one delivery boundary.

Completed deletion details remain at
`3296fcbab4a0a143e46fd7fcdfc18a72ca0c8a7c:scratch/keep-every-wave-ready-for.md`
under this heading. Shared accepted-fact projection, transactional registration,
checkout admission and exact-ID selection retain their behavior tests.

Compression moved the surviving Wave planning lock and Home-placement check from
`ops/chapter.rs` to `ops/pm.rs` (`lock_wave_planning`, `require_planning_home`).
Ordinary ensure, binding, editing, refresh, registration and reteam no longer
depend on chapter orchestration for these mechanisms. Keep the existing lock
namespace and queued-writer ownership so running callers retain exclusion.

- `ops/chapter.rs::select_current`: remaining rotation/legacy-adoption status selection awaits
  exact transitions. Ordinary selection moved to `ops/project.rs`; status Task
  filtering and Desktop status inference are deleted. `sync_projects` and
  `record_project` are deleted.

- `plan_rotation`'s `predecessor_names` and target-name lookup, plus `successor_id`:
  replace inferred predecessors and name-derived UUIDs with exact recorded IDs.

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

Shared acquisition, first-error collection, membership and migration-slice
boundaries remain required. Prior compression detail survives at
`a7789f1b88beb125110b36f511510701c463fd1d:scratch/keep-every-wave-ready-for.md`.
CI repair retains outer admission without reacquiring it in reservation.

The recorded nextest pass also reports a leaky projectless-Task case. Its cause
is unknown; gate retains output-handle investigation, not an assumed harmless leak.
Cached-name conversion and shared selection must land together. The approved
historical name-only exception is implemented; rotation selectors remain.

Reserve identity before provider effects so a timeout cannot create another UUID.
Use status-only writes and byte-preservation tests to protect authored content.
Review rejected bootstrap chapters, creation during status reads and name-derived
permanent Project IDs. Keep recovery receipts confined to mutation recovery;
provider status/content and configured selection retain their respective owners.

Release integration detail remains at `1e5086aa3:scratch/keep-every-wave-ready-for.md`.
#1451 preserves the name-cutover fixture's same-batch boundary; source v0.13.4
preparation is not publication. Release's child memory still records manual
v0.13.3 verification, unresolved unattended settlements and the operation-entry
recovery obligation for ensure/rotation.

Release's operation-entry recovery lesson still applies; its publication and
installation evidence establishes no Project-readiness acceptance. Registration
compression and prior check evidence remain at `eb80d7198178a418cf44b3074f9b666f3e11cd40`.

Checks: prior build, `project_ensure` (7), `project_binding` (5), `cancelled_` (6), formatting and Clippy passes retained; prose realignment: `git diff --check` passed. Gate retains configured/CLI/Desktop acceptance, the prior output-handle leak and CI repair.
