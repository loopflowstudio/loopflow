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

## Selection ownership — accepted October 6

Jack Heart authorized the reactive Project redesign in the independent pursue
request, superseding the Home-local YAML implementation choice. The shared local
owner is `waves.current_project_id`, referencing the accepted SQLite Project.
All checkouts and both Desktop surfaces read it. The first explicit ensure,
bind-project or applied rotation imports a historical Home-local YAML selection
once after exact ownership validation; `project_binding_imports` retains original
bytes (including absence). Later file edits are inert. Reads never import.
The earlier file-owner implementation and its byte/race proofs remain at
`42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.

LOO-382's committed `e887a21c199fedfb1a873ae33148b356f6e57b11` is integrated
through `lf sync --manual` at `66a3daa86`. Its uncommitted edits were not imported.
The dependency's design and measurements remain at that revision under
`scratch/update-the-workspace-automatically-when.md`; its performance limits remain
unproved here. No stack parent or Task identity was replaced.

### Delete — do not maintain

Completed: `work/wave/project_binding.rs`, its YAML writer/exclusive tests,
`ProjectPreparation`, per-view activation generations, activation-owned refresh,
empty-Flow template UI and `yaml-edit` are removed. Do not restore those owners.
SQLite retains exact-ID selection and original import bytes; existing Wave/checkout
locks and transition evidence preserve uncertainty and Task/PR/Session/Flow identity.

Compression removed redundant Portfolio refresh/detail state, Roadmap action
wrappers and duplicate Task selection types. `WavePlanView` owns one stable
Task ID, scoped to its keyed Wave view. Persisted identity and authority are
unchanged. Prior rationale/checks: `90013bd7e5bac5a17a213b6eabb48a1f7292dde8:scratch/keep-every-wave-ready-for.md`;
the dirty cleanup is retained in `/tmp/loo366-gate/before.tar.gz`.

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
SQLite selection writer, then status-only activation through ensure and
provider readback. Binding setup needs an explicit supported operation alongside
ensure; its implemented CLI spelling is `lf wave bind-project <wave> <uuid> --json`. It must validate the
exact Project's ownership under the same Wave lock and preserve original import
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

October 6 reconciliation: Jack Heart's “lg2m” after the updated walkthrough
accepts the source/naming direction and completes that review boundary; the
[demo record](project-readiness-demo.md) retains the feedback and limits.
Its last recorded GitHub readback places PR #1463 at `457e65d6b`, superseding
`d3e0f2ec9`. Local HEAD `c1685f82b` merges main's v0.13.6 source (`e177eafe6`),
including canonicalized store revisions; it does not establish publication or
installation of this Task. The Project-readiness draft remains separate.
Three local Swift files contain the Roadmap action simplification, private
TaskSelection rename and Wave-scoped stable-ID selection cleanup. The recorded
33-test headless pass covers that cleanup, not the remaining acceptance below.

Registration, exact-ID readers/admission, ensure, rotation, Portfolio activation
and chapter skills are implemented. Their contracts and proof boundaries remain
below. Prior implementation chronology is retained at
`42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.

October 6's disposable demo exposed primary opening without ensure and empty-Flow
UI. The authorized reactive implementation now connects both surfaces through the
model and removes the absent template section. [Demo evidence](project-readiness-demo.md)
retains the original failure; the fixed mounted paths remain unproved.

Gate still owns cross-process CLI/crash proof, configured acceptance,
skill outcomes, output-handle leak investigation and CI-repair entry coverage.
Mounted opening/reopening/retry judgment remains with review. #1450's Session-ID conversation control and capture-history distinction
remain required. Source fixtures establish neither installation nor Intelligence
repair.

### Exact-ID rotation — October 5 implementation

`ChapterPlan` contains `name` and per-Wave `wave_id`, `successor_id`, `create`,
`project_name` and `content`. The required `--plan` file supplies the same entries
for `lf repo new-chapter <name>` and `lf wave new-chapter <wave> <name>`; the latter
consumes and validates only its Wave, independently of invalid sibling plans.
Names never select Projects. Newly authored destinations
use UUID v4, allocated once. Existing destinations must exist. No provider writes
precede whole-input KR, destination, conversion and Task-evidence preflight.

Each Wave's current shared binding supplies its predecessor. Exact settled lookup
recovers partially completed repository operations without choosing a current
Project from a receipt. All pairs are reserved before the first provider write.
The existing draft now retains transition-owned selected issue IDs and explicit
create/existing intent. This prevents a missing existing destination from becoming
a creation on retry. Authored KRs/targets can be corrected before the binding
switch without discarding reserved identity. A full-input digest was rejected in
source review because it would freeze a provider-rejected unfinished plan. Ensure
reservations need no reset creation flag; their operation always owns creation.

Preflight accepts fetched Project facts before checking status, so a stale read
cannot hide a newer completed destination. Only selected legacy endpoints undergo
conversion, without promotion of unrelated Projects. Conversion settlement and
membership writers retain their Wave/checkout guards after caller cancellation.
Existing successor names, summaries and unrelated Markdown survive planning-field
updates; only supplied KRs, targets and optional Flow change. Accepted readbacks
confirm activation and authored planning before the binding switches. Predecessor
completion follows it; the binding is read again before settlement. Failed readback
or an intervening binding leaves a retryable receipt.

Before the switch, recovery retains uncertain selections and reclassifies new
predecessor work. Afterward it checks only the saved issue IDs: later historical
starts stay put and external moves are conflicts. Task/PR/checkout/Flow identity
and unbound Session membership retain their existing owners. Settled retry cannot
reverse a later binding, including two explicit plans with the same display name.

The replacement operation fixtures cover every provider-mutation response loss,
partial repository settlement, created/existing successors, transfer-readback loss,
new work before/after switching, external post-switch moves, backlog retention,
authored text, empty predecessors, unknown work, checkout start exclusion and
cancellation around selected-membership persistence. Whole-input failures include
invalid KRs, absent/foreign destinations, invalid legacy content and newer accepted
terminal evidence. Their current check result is recorded at the end of this plan.
These are stateful loopback operation proofs, not separate-process CLI crashes or
configured acceptance. Cross-checkout CLI, actual crash and Desktop proof stay at gate.

The prior counterexamples and superseded name-based implementation remain at
`fc6df439424bd341ec3cd8182c19b13ed45cffd7:scratch/keep-every-wave-ready-for.md`.
Release's operation-entry lesson remains applicable; no installed or production
Project readiness follows from these fixtures.

### Accepted planning owns durable projection

Atomic ingestion, per-entity acquisition age, independent Initiative/Team ownership,
cancellation-safe Wave/checkout guards and registration remain required. Their writer
inventory and counterexamples remain at
`42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.
Partial accepted Projects now render before full inventory sync. They retain their
own age and ownership; the Project list is marked partial and unknown Task inventory
stays unknown. No full-sync timestamp is manufactured by ensure.

### Created-successor recovery

The old name-selector failure on `A — next, A — previous` remains at
`fc6df439424bd341ec3cd8182c19b13ed45cffd7:scratch/keep-every-wave-ready-for.md`.
The replacement operation loses a transfer response/readback and retries the exact
reserved UUID, preserving Task/PR/Flow identity. Different predecessor names need
no shared-name inference. Cross-process interruption remains gate acceptance.

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
  reconciliation. The rotation fixtures now exercise the shared-binding switch; CLI crash proof remains at gate.
- **Started is not complete work evidence.** The earlier `chapter_task_evidence`
  read only Started and the managed worker claim. An unbound, non-primary conversation
  under a Task checkout can have neither, even without concurrency.
  `reserve_session_in` correctly leaves its attribution null. Classification
  now reuses shared Task-work Session membership and started mechanical Flow
  history, including primary-scope exclusion. That closes the static reader gap;
  rotation stabilizes membership with checkout exclusion. A second path counter
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
checkout exclusion. Rotation now includes both sides of the
configuration switch; CI repair's operation-entry proof remains with gate.

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
Rotation adds both start orderings and failed-reset retry. Operation fixtures cover configuration switching
and created-successor recovery; CLI crash proof remains with gate.

## Outcome and demo

Every Wave has one explicitly configured current Project, normally In Progress.
Opening a Wave in Desktop ensures that Project independently of any chapter. Existing
names, content, Tasks, checkout, PR, Session and Flow identities survive. CLI and
agents use the implemented `lf wave ensure <wave> --json`. Status,
roadmap, background refresh and Project inspection never initiate provisioning.
An unavailable sibling Wave or global reset does not prevent ordinary work here.

On opening, show retained Project and Tasks while preparation runs. With no
retained plan, show “Preparing Project…” while the local activation command is
pending. Both surfaces now derive this presentation from the model's existing
per-Wave command handle, including Portfolio before its first workspace frame.
Cached planning remains visible, Retry waits for completion, and current transport
failures take precedence over cached activation errors. An unfinished persisted
Exec without a local command still means unknown; no persisted lifecycle is added. An outage
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

`work/wave/config.rs` retains authored policy; `store/sqlite/project_selection` owns shared
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
`WorkModel` now restores the saved workspace and retains last-good planning on read
failure; its planning and Session refresh loops run independently. `WavesView`
now creates its window model lazily. `WaveDetailPane` separately polls status
and renders `reading.plan(cached: plan)`. The model dispatches opening/Retry; both surfaces consume the Work stream.

Keep activation at the explicit Wave-opening/retry boundary through the shared
transport, outside model construction, cache restoration and both periodic read
owners. Preserve the existing saved-plan presentation and independent Session
refresh while ensure runs. Headless Desktop acceptance must include reopening
from saved state with failed or delayed ensure, and switching Waves before that
response returns. #1454's benchmark integration changes no activation. First-frame timing stays
separate from ensure completion; no new launch benchmark is required. Earlier
measurement limits remain at `1e5086aa3:scratch/keep-every-wave-ready-for.md`.

## Chosen operation and authoritative state

`lf wave ensure <wave> --json` is the explicit activation operation shared by
Desktop and agents. Under the Wave planning lock, import a legacy binding once,
then resolve its exact Project, validate accepted ownership/status and activate
Backlog/Planned through status-only writes. Completed, archived, paused, foreign
or inaccessible evidence never authorizes replacement. Names and empty Flow do
not select or reject ordinary Projects.

With no selection, reserve one UUID before provider effects. Retry create, attach
and activate through that UUID. Confirm accepted facts, then atomically select its
local Project and settle creation. Readiness derives from accepted facts, selected
identity and unresolved transitions. Original acquisition times stay visible.
`waves.project_activation_exec_id` associates the actual ensure process; `execs`
owns its outcome/error. No terminal receipt means unknown, never inferred running.

`bind-project` validates and selects an existing exact Project without activation.
The YAML import records original bytes and its one-time completion transactionally;
malformed files/provider errors retain the pending import and prevent creation.
Afterward old YAML is inert and other policy bytes remain untouched. Selection-only
commits wake LOO-382's planning revision, as do recovery-record changes. Watchers,
status, roadmap and DB reads remain observational.

Explicit opening and Retry dispatch from either Desktop surface through the model.
Closing a view cannot cancel the command. Both surfaces render streamed readings;
CLI commits reach them without view-owned rereads. Existing scope/sequence/answers
protection rejects stale frames. Swift owns selection/drafts and command transport
feedback, not another Project lifecycle. Primary Session preparation is independent.

## Persist only the missing recovery evidence

Add one narrow Project transition receipt in the existing SQLite owner, using
one migration draft for this Task. Receipt fields: Wave ID, reserved successor
provider ID, optional predecessor provider ID, optional explicit reset name,
creation timestamp, settlement timestamp and optional reset create/existing intent. A unique unfinished transition per
Wave serializes admission alongside the Wave lock. For reset, persist the exact
selected predecessor/successor before effects; for initial ensure, predecessor
is null. Existing Project facts remain in their existing tables. Retain selected issue IDs as transition-owned membership. Retained reset intent
prevents an existing destination from becoming a creation on retry; it selects nothing. Do not copy Task bodies, KRs, provider status or
execution cursors into this receipt.

This is operation recovery evidence, not a Chapter object or second plan. Fresh
provider observations determine the next missing effect; a phase counter must
not assert an effect happened. Readback settles the receipt only when attachment,
the SQLite selection, and any predecessor completion are confirmed. Preserve it
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
complete the predecessor. Persist both IDs before effects; retry accepts SQLite selection
matching either endpoint and reconciles the missing effects. A different config
ID is an intervening decision, not permission to overwrite it. SQLite selection and provider
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
to destination Project IDs after creation. The implemented skills use `scratch/chapter-plan.json` and separate
`scratch/chapter-candidates.md` notes grouped by destination UUID, with stable
local keys and recovered issue IDs. No complete future Task inventory is required.

Preserving existing work belongs to rotation, not the optional creation of new
Tasks. Review and planning account for old KRs and backlog; neither an empty new
Project nor wholesale old-Task cancellation establishes a successful transition.
Initial KRs are required for chapter creation but can evolve through ordinary
Project edits afterward. Ordinary non-chapter Project ensure stays independent
of this review/planning sequence and may create a Project without KRs.

## Chapter facilities and history

Wave-scoped creation uses repository rotation's exact-ID operation. Cross-Wave
history inspection remains proposed: metadata representation and projection are
unresolved. Preserve renamed/completed Project IDs; never infer membership from
names. Full proposal: `42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.

## Implementation and deletion history

Earlier steps and deletion rationale remain at
`42e6c2706b1b35b2852e438ff94a49d060faccda:scratch/keep-every-wave-ready-for.md`.
Cancellation-safe guards, exact-ID readers and rotation proofs are retained above.
Desktop keeps its preview report/input on Apply failure; only a new Preview
rereads the file. Mounted proof remains unfinished. Migration-marked conversion
serves released-data preservation only.

## Reactive implementation — October 6

Jack Heart's pursue request authorizes this implementation; the earlier review
proposal is preserved in `/tmp/loo366-pursue/review-before-worker.tar.gz` alongside
the supervising conversation's untouched walkthrough. SQLite selection, one-time
import, derived readiness and both Desktop activation/read paths are implemented.
Creation selection and settlement commit together. Rotation preserves its existing
pre/post-switch recovery boundary. Accepted Project reads do not require inventing
a completed full inventory sync.

Source review corrected two remaining boundaries: partial accepted Project facts
must render before full inventory sync, and rotation's final selection check belongs
inside its settlement UPDATE. Primary opening waits for the selected Wave to resolve
from retained/streamed planning; command lifetime remains with the model. The deleted
YAML editor also removes `yaml-edit` and its exclusive dependencies.

Proofs: a selection-only commit reached two public workspace-watch processes; Project
and rotation fixtures preserve identity and failure recovery; headless model tests
retain planning through failure/navigation and suppress empty-Flow presentation.
These are disposable source proofs, not configured or mounted acceptance.

Pending-command presentation is implemented through the existing per-Wave transport
handle. Headless checks cover retained planning in both surfaces, delay, failure/retry,
navigation and unfinished persisted Exec uncertainty.
Remaining proofs: actual configured Intelligence repair after publication/install;
public CLI ensure with simulated provider, actual process crashes across rotation;
mounted opening/reopening/retry in both surfaces; skill-driven chapter/admission
outcomes; retained output-handle leak and CI-repair operation-entry coverage. The
existing source demo did not exercise public CLI dispatch or real Linear.

## Acceptance at gate

Ensure/rotation operation fixtures and focused Desktop tests already exist.
The cases below retain the full acceptance contract, not a claim that every case
has passed. `rust/loopflow/tests/project_readiness.rs` is still absent; its
cross-process CLI and crash cases remain to be implemented and run at gate.
Use the repository's isolated fixture environment and compiled test CLI; prevent
native providers and configured accounts from being launched by synthetic tests.

- `cargo test -p loopflow --lib project_ensure`: no Project → one Started record;
  repeat/two processes → identical UUID and one provider Project; ordinary
  configured Backlog and current no-Flow Projects preserve all bytes/IDs;
  Task resolution, sync preview/apply and reteam retain ordinary provider names;
  configured identity is independent of other Project names/statuses; absent
  configuration creates and records one ID without candidate discovery; failed
  access produces no provider writes; interrupted create/attach/activation
  resumes exact UUID; archived pending identity is not recreated. Failed selection
  transactions retry the same identity; intervening selections survive. Assert outcomes,
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
  A malformed legacy import causes no provider writes; read-only access imports
  nothing. Preserve original YAML bytes, prove import once and selection-only stream
  invalidation, and retain the same selection across a Wave rename.
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
historical name-only exception and exact-ID rotation are implemented.

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

Registration compression evidence: `eb80d7198178a418cf44b3074f9b666f3e11cd40`.

Earlier reactive implementation checks remain at `fd98f97352fdfc16145adf0efcc045443ae3a824:scratch/keep-every-wave-ready-for.md`; logs: `/tmp/loo366-pursue/`.

Earlier compression and pending-presentation checks (28 and 40 headless tests,
including repaired initial failures) remain at
`457e65d6be132f290b8968c4b11959a1ca0823e6:scratch/keep-every-wave-ready-for.md`;
logs remain under `/tmp/loo366-compress-detail*` and `/tmp/loo366-pending-tests-retry.log`.
Current transport feedback takes precedence over retained activation failure.
CLI/crash, configured and mounted acceptance remain open as described above.

## Naming direction — October 6

Jack Heart selected Loopflow Desktop, superseding Podium, and authorized this
independent naming implementation. The shared observable state is `WorkModel`;
`RepoView` shows one selected repository while the model also serves Portfolio.
Work observation, frames, requests, scope, navigation, projection, cache and their
consumers now use Work names. The CLI stream is `lf monitor work --watch --json`;
Rust/Swift callers, fixtures, filenames, tests, current docs and tooling move together.
Task/Wave presentation joins are `TaskProjection` and `WaveProjection`. They add
no durable Work kinds: Wave/Project/Task remain the owners, with associated Sessions
and Execs. Loading uses `WorkReadingStatus`, distinct from durable `WorkStatus`.

Delete — do not maintain: the Podium-prefixed implementation, Workspace-prefixed
observation/navigation/projection APIs and `monitor workspace` spelling. No alias
or parallel reader remains. Historical releases, measurements and the supervisor's
review artifacts retain their original names and provenance.

`TaskWorktreeSnapshot` describes Git placement. `WorkspaceIdentity` and
`SessionWorkspace` retain their names: primary conversation paths can be non-Git
directories. Terminal workspaces own panes and broader working environments;
`NSWorkspace` is Apple's API. The existing `workspace.json` cache, envelope version,
selection/layout preference keys, placement JSON fields and metric identifiers
remain unchanged. No migration or identity churn is introduced. A fixed pre-rename
cache payload proves retained Home/Task selection and unchanged file bytes.

The stream fixture now seeds its accepted Project selection before registering
a Task, fixing the pre-existing admission failure without weakening watcher
assertions. Architecture declarations now include the existing readiness tables
and historical YAML import. Neither correction changes production behavior.

Naming checkpoint: `90013bd7e5bac5a17a213b6eabb48a1f7292dde8`, integrated with
main at `457e65d6be132f290b8968c4b11959a1ca0823e6`;
the later publication and accepted review are recorded under “Remaining coherent
cut.” The supervisor's review artifacts remain separate. Broader gate,
configured Intelligence, CLI/crash and mounted acceptance above remain open.
No production provider mutation, installed-Home access, installation or release
belongs to this naming contribution.

Earlier naming checks and compression results (33 Rust, 139 Swift, five and
33 focused Swift tests), plus the one-test two-reader proof, remain in the
pre-gate plan at `/tmp/loo366-gate/before.tar.gz` and their original logs under
`/tmp/loo366-naming/`, `/tmp/loo366-compress-naming/`,
`/tmp/loo366-compress-final/` and `/tmp/loo366-sync-work-watch.log`.
They are historical passes, not a gate for the current tree.

## Gate availability — October 6

Gate at `c1685f82b` preserved the existing dirty cleanup and review artifacts at
`/tmp/loo366-gate/before.tar.gz`. Source inspection covered ensure/binding,
rotation preflight, SQLite selection/readiness, CLI dispatch and Desktop command
ownership/selection. It found no demonstrated repair in those inspected paths;
this is not a complete review or a passing gate.

The resource preflight failed: supported recovery removed 3.7 GiB of eligible
inactive build artifacts but left 17.0 GiB free against the required 32 GiB
reserve. A standalone zero-wait uv prune also found its cache locked. Active and
recent builds remain intact. Product suites, Clippy and app builds did not run;
prior passes do not establish this tree's gate. Logs: `/tmp/loo366-gate/`.

The public CLI/crash suite remains absent. Existing `task_deletion_tests.rs` and
`tests/e2e/task_deletion.py` demonstrate a Linux-only TLS proxy using fixture CA
trust without changing the production endpoint; macOS does not honor that fixture
trust path. This is a possible acceptance harness, not an implemented or executed
Project-readiness proof. Operation fixtures do not cover public dispatch or
process crashes. Skill/admission outcomes, output-handle leak investigation and
CI-repair entry coverage remain open. Configured Intelligence acceptance still
requires the published commands; no installed Home or provider was mutated.

Headless Desktop model/view tests and app builds are the automated boundary;
display-dependent mounted judgment belongs to optional demo/review, not a gate
prerequisite. Configured provider acceptance remains separate and unproved.

Gate check: `cargo fmt --all -- --check`, `uv run python scripts/check_architecture.py`, `uv run python scripts/check_migrations.py`, `uv run python scripts/check_swift_multiplatform_boundaries.py`, and `git diff --check` passed; affected Rust/Python/website/Swift suites and Clippy deferred to a capable gate/CI because resource recovery failed; CLI/crash acceptance remains unimplemented.
