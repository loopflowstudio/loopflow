# Planning

```bash
lf --wave product wave/operate
lf task status INF-124 --json
lf checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
lf repo new-chapter 2026-10 --plan scratch/chapter.json --dry-run
```

Wave → Task is the navigation hierarchy. A Wave keeps its objective, memory,
cadence, budget and metric instruments across plans. Its one In Progress
Project owns Tasks, KRs, targets and an optional workflow. A Chapter is the shared
name of those current Projects across the repository. The rotation below describes
the explicit shared Project binding. Rotation retains exact destinations and selected
issue IDs for recovery, while unreviewed backlog stays in its original Project.
[Cutover status](../architecture-reference.md#cutover-status) records
other implementation and proof gaps.

## Planning authority and identity

```bash
lf task create --title "Fix the parser" --json
lf task comment <id> "Keep quoted input"
lf checkout <id>
```

Explicit creation without a Wave provisions `personal:inbox` and its Project.
Personal plans bind a canonical repository in SQLite. Their Waves, nested definitions,
goals, memory, workflows and comments remain there; reads provision nothing and no
personal definition is written into the code checkout. Explicitly connected Waves
retain Linear authority and repository-owned definitions. Authority follows stored
Wave ownership, never the presence of an external alias.

Task and Project rows own identity for both paths. Random UUIDs are minted before
placement; provider UUIDs and ticket aliases remain optional mappings. Accepted
owned Linear issues become durable, unplaced Tasks in the ingestion transaction.
Repeated import and ticket changes retain identity. Unowned observations stay in
the provider evidence tables; orphan deletion identities remain recovery evidence.
The released-frontier migration preserves existing identities, serialized provider
observations, PRs, workflows and Session links.

`--creation-id` identifies one creation request. Its transactional receipt retains
the original Project and input through later edits and rotation. Independent requests
with identical titles remain distinct. The same transaction owns local field edits,
assignment, Project ordering and comments. Revisions are local write concurrency,
not fabricated provider observation times. Local Task selection displays the shortest
unique UUID prefix of at least seven digits; full IDs remain stable. References
accept four or more case-insensitive hex digits, bare or after `lf-`/`task_`;
ambiguous references return candidates instead of selecting a Task.

Saved Tasks use the same checkout preparation and placement transaction with or
without Linear. Checkout reads retained planning and branch metadata; it does not
refresh Linear. An unknown provider alias still needs initial acquisition.
Preparation uses the saved Task identity and title directly. New branch names retain
the full Task UUID; default checkout names use up to four title words and an ID
suffix. Retained provider branch names take precedence.
Placement records the first PR and checkout before filesystem creation, without rewriting
planning fields. Accepted state and completion are retained on the Task, so terminal
planning prevents first placement even without provider inventory. Task and Flow entry
use the same SQLite admission reader as placement, without a provider resolver.
It reads one snapshot of saved planning, Workflow state and retained invalidation,
removal and membership evidence. Missing provider inventory alone does not prevent
a saved Task from running; a retained invalidation still does. Remote completion
prevents new work but does not stop an already active Workflow. Incoming removal
retains the Task and marks its planning deleted; neither observation moves a Workflow
or cleans a checkout. Competing reservations return the saved allocation; restoration
uses that checkout and PR. The queued writer retains the Wave lock through commit,
even if its caller is canceled. Native launch and resume retain the
ordinary Session and Process authority checks. Local deletion receipts and retained
provider deletion evidence apply regardless of planning mode. Local completion
records its decision and workflow arrival together.
GitHub delivery still requires confirmed merge evidence. Local chapter rotation commits
all selected personal Waves together, carrying started Tasks and preserving backlog.
A mixed Local/Linear rotation retains each authority's recovery records; a SQLite
transaction cannot make the external provider effects atomic.

LOO-406's accepted replacement is one locally owned plan with repository-wide
optional Linear synchronization. That cutover remains unfinished. Creation, edits,
definitions and rotation retain separate planning authorities;
the common offline checkout and launch paths do not establish common planning ownership.
Project content and stored Wave workflows share `sqlite/project_content.rs`;
comments use `sqlite/task_comments.rs`. `sqlite/plan_read.rs` supplies registered Task status
and Wave/Desktop planning from saved Task and Project rows. Reads preserve state,
completion time, ordering, assignee, provider metadata and observation age separately
from Workflow position. Missing provider inventory cannot erase saved planning;
malformed observations retain fields and invalidation diagnostics. Pending local
state decisions survive inbound completion while other fields advance. Ingestion
and the released-frontier migration retain editable Project content, including KRs
and targets. Current Project selection and readiness also read saved fields and
observation age without personal/Linear dispatch. Missing provider inventory keeps
that selection readable; retained archival, invalidation or membership conflicts
still make it unavailable. These local guarantees do not fix Linear's unconditional
update race.

## Connected Project selection

```bash
lf wave bind-project product <project-uuid> --json
lf wave ensure product --json
```

Validate one existing Project and select its accepted local record on the Wave's
SQLite row (`waves.current_project_id`). Repeating the same binding preserves it;
a different existing binding is left unchanged. Setup preserves provider status,
content and identity, including a Backlog Project without a workflow. It does not
activate the Project. Operation routing and status select this exact ID; the JSON
Project summary carries a required `current` boolean for Desktop. Status and
roadmap retain predecessor Projects and Tasks. Registration and unstarted managed
work require that exact Project to be In Progress; another In Progress Project
does not compete with the binding. Already-started Tasks retain continuation in
predecessor Projects.

`ensure` activates the configured Backlog or Planned Project with a status-only
write. Without a binding it reserves one UUID in SQLite before creation, attaches
it to the Wave's Initiative, activates it, then commits selection and creation settlement together. Retry
reuses the reservation across uncertain responses and failed binding writes.
Terminal, archived, paused or foreign Projects report their condition without
replacement. Names, content and an empty workflow remain intact. Ensure neither searches
for candidates nor performs rotation; status and roadmap never call it. Desktop
ensures on explicit opening or retry while retaining cached planning and independent
Session reads. Both primary and Portfolio surfaces render the same SQLite-derived
`project_readiness` through `lf monitor work --watch --json`. Committed selection,
accepted facts, transitions and exact activation Process outcomes invalidate that reading.
A pending transition or unfinished Process is unresolved evidence, not proof of liveness.
Watchers and status reads never provision or import.

The first explicit ensure, binding setup or applied rotation imports an existing
`<Machine>/waves/<WaveId>/config.yaml` selection once, after exact provider ownership
validation. SQLite commits the selected Project and original YAML bytes together in
`project_binding_imports`; absent files are recorded too. Malformed files or failed
provider reads leave import retryable and prohibit creation. After import, the file
is inert, even if a stale checkout or old writer changes it. Other YAML bytes remain
untouched. An imported completed Project stays selected history and cannot be reopened.
Activation records its exact Process on the Wave; that Process remains the sole owner of
its terminal outcome and error. Swift retains command transport feedback only.

## Rotate the plan, preserve the work

```bash
lf repo new-chapter 2026-10 --plan scratch/chapter.json --dry-run --json
lf repo new-chapter 2026-10 --plan scratch/chapter.json --json
lf wave new-chapter product 2026-10 --plan scratch/chapter.json --json
```

Retain one JSON input with `name` and `waves`. Each entry supplies `wave_id`,
`successor_id`, `create`, `project_name` and `content`:

```json
{
  "name": "2026-10",
  "waves": [{
    "wave_id": "<registered-wave-id>",
    "successor_id": "<new-or-existing-project-uuid>",
    "create": true,
    "project_name": "Autumn customer work",
    "content": {
      "workflow": "",
      "metric_targets": [],
      "krs": [{"text": "Customers can resume unfinished work", "holds": false}]
    }
  }]
}
```

Allocate a new UUID once when authoring a creation plan. Set `create` to false
for an existing exact destination; absence never turns that instruction into
creation. Every destination requires authored KRs before any provider write.
Existing Projects keep their names, summaries and unrelated content; supplied
KRs, targets and optional workflow replace only those planning fields.

The repository operation validates all selected Waves, destinations, legacy
conversions and Task evidence before reserving every pair and applying the first
Wave. The Wave command consumes only its entry through the same operation.
The shared binding identifies the predecessor. Names never select either endpoint.
Started unfinished Tasks move with identity, checkout, PR and captured execution
intact. Unreviewed backlog remains historical; unknown evidence blocks apply.

Retry with the same file. SQLite retains exact endpoints, create/existing intent, and
selected issue IDs; those records never select the current Project. Before the
binding switch, retry permits corrected planning fields and reclassifies new work. After it, only retained
selected issues are reconciled; new historical starts stay put and external moves
remain conflicts. Predecessor completion follows the binding switch. Settled
Waves can be retried alongside unfinished ones without new Projects, and an old
plan cannot overwrite an intervening binding. No Chapter table is required.

Task candidate admission follows Project creation separately. Retain those
candidates alongside the input and use the existing Task creation operation;
failure there does not undo the selected Projects or their KRs.

## Inspect planning before starting execution

```bash
lf task status INF-124 --json
```

Status returns `planning`, `planning_state`, `planning_error`, `planning_stale`,
and optional `execution`. An issue
without a Project stays inspectable; operations requiring ownership report the
missing relationship. Status may refresh planning and observe a PR, but never
completes a Task. Reading an issue allocates no Task execution or worktree.

Exact lookups and Wave views share normalized SQLite entities scoped to a
repository and provider. An empty cache fetches the requested issue before
reporting absence; provider errors remain resolution failures. Bounded refresh
retains the last successful observation and its timestamp on failure. Lists
reference shared Project/Task facts; omission alone never proves deletion.
A null detail response invalidates cached admission until a complete detail
read repairs it. Confirmed deletion receipts continue to exclude removed Tasks,
including after stale list ingestion.

Status preserves dated facts even after hard-stale or forced acquisition fails.
`planning_state` distinguishes `available`, `unavailable` acquisition, `invalid`
evidence, confirmed `removed` planning, and an `absent` detail response. A null
provider response observes absence for that acquisition; the stored record stays
invalid until complete detail repairs it. This creates no deletion receipt.
`planning.observed_at` remains the last successful acquisition. An unresolved
selector returns an unavailable envelope even without execution.

Invalid and removed facts remain inspectable through status. Task execution
requires available planning: a fresh cached observation suffices, but a failed
due refresh stops the launch. Wave `synced_at` dates its last successful list
acquisition; joined entities may have newer detail observations.

Task facts carry Linear's `updatedAt` as `revision`. Detail, list and confirmed
mutation refreshes share one writer: older provider revisions cannot overwrite
newer facts, and conflicting facts at an equal revision fail without replacement.
Complete responses must include nullable fields; omission cannot clear known data.
Stored change receipts invalidate planning even without execution. A complete read
at or beyond the receipt's revision repairs the invalidation. Removal receipts fence
later reads, including when the receipt arrived before the issue was cached.
Receipts do not replace complete planning entities. The former daemon webhook
ingress is removed; these store operations do not establish live event delivery. Project facts also
carry `revision`; a newer Project observation updates independently of the issue's
revision. Older observations cannot overwrite it. Project responses must include
nullable content fields and relationship sets.

Snapshot and detail ingestion project accepted entities into durable Project and
Task rows in the same SQLite transaction. Projection uses each stored entity's
acquisition time, never the Wave's aggregate sync time. A projection failure rolls
back observation acceptance. It preserves execution fields and retained identity;
an unknown destination does not authorize a Task transfer. Restart changes
execution without rewriting accepted planning. Rotation accepts confirmed Project
and transfer readbacks through the same owner. A single Project observation does
not advance the Wave's full-refresh timestamp. Reteam accepts complete issue
readbacks through that same owner and reconciles confirmed Team relationships
without changing Initiative ownership or independently newer Project facts.

Full and partial Wave ingestion validate the accepted Project's Initiative before
recording membership or freshness. Detail refresh resolves that Initiative through
Wave configuration and the registry, then accepts the association and projects
facts in one transaction. A durable Project row alone grants no Wave ownership.
Unconfirmed cold detail retains durable plans; foreign or unmapped ownership at
the operation boundary reports an error. Both Project and Task projection require
an exact accepted Initiative match. Snapshot acquisition and cold-detail readback
hold the Wave planning lock through SQLite acceptance. Queued workers retain
shared ownership after their async caller is canceled. Reteam and rotation acquire
participating Wave locks in stable ID order; already-held refresh paths reuse them.
Cold detail discovers ownership, locks, then reads ownership again. Null detail
invalidates only the cached revision and observation it queried; it cannot invalidate
a newer accepted Task. Readback follows the issue UUID across identifier changes.

The `project_readiness` migration preserves original Linear Project bodies and
acquisition evidence before a one-time name/slug correction from fresh provider
facts. Only pre-cutover rows receive this exception; all non-name conflict checks
remain, and later equal-revision name conflicts are rejected. Rejected ingestion
cannot consume the exception or update durable Project facts first.

List coverage never removes a Project merely because a later response omits it.
Without removal evidence, refresh fails and retains the previous observation.
Project revisions do not establish ordering for separate Initiative/Team
relationships. A contradictory relationship set stays unresolved, retains its
last-good facts, and blocks managed readers. Replaying a list or detail does not
clear that uncertainty. Explicit reteam reconciles the exact confirmed Team set
under acquisition ownership; it cannot reconcile a changed Initiative.

Chapter rollover transfers started work and retains unreviewed backlog in its
predecessor Project before completing that Project in Linear. It confirms provider
completion before recording the Project. Closing it neither completes transferred
Tasks nor changes historical KR results. Current Project readers follow the shared
binding, which rotation switches before confirming predecessor completion.

The planning store can retain explicit archival acknowledgements. Integrating
archival into the provider-backed chapter operation and preserving old acknowledgements
across the combined migration frontier remain unfinished.

## Capture a Flow once

```yaml
# .lf/flows/build.yaml
- step: implement
- step: compress
- step: gate
```

```bash
lf flow build
lf flow show DRIVER_PROCESS --processes --json
```

A Flow is one lf process and the step processes it starts; its ID is the driver
Process's. Starting one compiles its definition, including every router and
alternative, into a graph the driver holds in memory. One cursor and its return
counters identify loop passes. Subflows and passes are display lenses over the
step processes, with no separate record or driver.

Every Flow naming a Task, or run in its checkout, is equally that Task's
work; none is selected or privileged. Taskless and Task execution share the
driver. Each `task run` starts a fresh Flow as a child `lf run`, and a fresh
one again when that Flow process fails. Flows hold autonomous
steps only: launching one with a `human: true` step is rejected. Finishing
retains history and chooses no successor; Flow completion alone does not
complete Task Work.

## Move a Task through its workflow

A [workflow](../authoring.md#workflows) is the outer shape of a Task: nodes
where a person takes part in the Task conversation, and edges that each run
one Flow. A Task takes up its Project's on its first `lf task run`, or the one
named; a Task that named its own keeps it. `task_workflows` holds one
row per Task: the graph as it was then, never edited, and the Task's
position. A later edit to the YAML applies to Tasks that take it up
afterwards; taking one up again starts over at `start`.

Position is stored: at a node, or on an edge with the Process carrying it.
Whether that Process still runs is read from the Process, never stored. The store
has one read and four writes, and every write appends to
`task_workflow_moves`:

| Call | Effect |
| --- | --- |
| take up | store the graph at `start` |
| choose | from the edge's `from` node, or from a stopped edge that left it: on the edge, carried by the choosing Process; straight to `to` for an edge that runs nothing. Refused when the Task has moved since it was read |
| arrive | the Process that carried the edge, on success, puts the Task at `to` if it is still on that edge |
| set | put the Task at a named node |

`lf task run ISSUE [NAME]` chooses, then runs the edge's Flow in the same
process, which arrives when the Flow succeeds. A Flow that stops leaves the
Task on its edge. `lf task move ISSUE NODE` sets. An edge is not chosen
while another runs. A move's author is its Process's calling conversation, else
a person; an arrival is the edge's own.

A Task's state is read from that position and stored nowhere else: not ready
with no Workflow, ready at `start`, active at a node or on an edge, done at
`end`. Abandoned is the Task's own mark. Any move that reaches `end` is
completion: one transaction writes the position, reason, Completed event, pending
delivery identity and retirement of an empty unpublished PR after the delivery
checks. Linear I/O follows independently. Reopening queues its own delivery in the
same transaction as the explicit Workflow move. A Task with no Workflow
reaches `end` on `unplanned`, which has nothing between. Linear calling an
active Task complete is read as `planning_conflict`; `end` then takes `--force`,
kept in the move's note.

Foreground Task connections acquire repository membership/state independently of
comment and state delivery. Delivery receipts retain their original provider revision,
attempt evidence and conflicting value. The latest receipt determines the displayed
writeback state; Tasks store no separate copy. A response records only its captured
effect, so it cannot settle a newer decision. Explicit state resolution supersedes
the delivery identity without changing the Workflow. No local/provider clock
comparison orders edits. The provider read and mutation remain separate requests;
they do not prevent a concurrent Linear edit between them. The complete unified
planning-owner cutover remains in progress; creation, edits and rotation still use
split paths. Abandonment of an existing Task saves the decision and cancellation
receipt together without provider access. Cleanup failures preserve the decision;
unknown or live execution prevents cleanup before provider inspection. Cancellation
remains pending until safe provider delivery is implemented. Delivery and pending-state projection currently require an
existing mapping; pending export of unmapped connected Tasks remains unfinished.

## Read each step's result

Each executed skill or operation runs in its own child lf Process as the plain
command: `lf -b skill <name> [message]`, or the operation's own command. The
step is told nothing about its Flow. The driver owns navigation and the Flow's
record, FlowProcess: the Flow's name and graph as compiled at launch, then one
appended row per step with its Process, graph node key and per-edge iterations.
Nothing reads it back to resume. A step's result is how its process exited; a
deciding or routing step also answers through the final answer of the Session
turn its Process captured. After an operation the driver stops the Flow when a
landing of its checkout is still being watched: neither failed.

A deciding step's message asks for a JSON `decision`: `advance` or `iterate`
with a nonempty `summary`, or `blocked` with a nonempty `reason`; the unused
field is null. A router's message asks for a JSON object containing `path`, one
of the branch's path names. The contract travels in the message, not as a
provider schema, and the driver accepts the value inside prose or a code fence.
Session history retains the native output and completion separately. An invalid
answer is corrected by `lf -b session resume ID MESSAGE` in the same
conversation, at most twice, then the Flow fails; provider failure remains
distinct.

Blocked records the reason and stops at the current Flow position. Existing logs
and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. Nothing retries
or resumes a stopped Flow.

## Inspect before further work

```bash
lf task status INF-124
lf flow show DRIVER_PROCESS --processes --json
lf task interrupt INF-124
lf task run INF-124 --reason "take the smaller approach"
```

A killed driver leaves its Processes and FlowProcess rows as history; the last step
row shows where it stood. Nothing restarts or resumes it. A Flow is `current` while its
driver has no recorded exit, `completed` when the driver succeeded and `stopped`
when it exited before the last step. Crash recovery
belongs to the caller, normally the Task conversation: inspect the Session,
process and effect receipts, then launch fresh work. Observation does not replay
work or consume a surviving child's result.

`task run` places the Task, then runs a fresh ordinary Flow in its checkout:
the same command, checks and records as `lf --task ISSUE run FLOW` or a run
from the worktree. It blocks until the Flow ends; a caller that will not wait
backgrounds it. `--reason` publishes direction to the Task first. To change
direction, interrupt, inspect, then run.

Task status and roadmap `execution` observe the Task's most recently launched
Flow: `none`, `latest` or `finished`. Its only control is `start`, available
whenever the Task can run, even when an earlier Flow exists.

Automatic provider retries within one Process retain earlier failure and usage and
select only an authorized successor. An uncertain mechanical effect requires
inspection; moving a cursor does not establish exactly-once external effects.
Missing process evidence is uncertainty, and causal ancestry grants no signal
authority.

## Task conversations

```bash
lf session list --json
lf session connect SESSION --json
```

Discuss design and review feedback in the ongoing Task conversation. Historical
review feedback remains Session evidence. No Ready/Complete operation closes the
conversation or releases a Flow. The caller inspects outcomes and effects
before selecting further work; pane closure and provider exit grant no authority.

Saved handoffs retain executable, Machine and database together. Renaming, binding
and driver replacement retain conversation identity and feedback. Desktop reads
the same record and keys its terminal surface on AgentSession identity.

## Work, steering and execution

Task status is `Ready`, `Done` or `Abandoned`. Current activity, command outcome,
conversation state and Flow progress remain separate. A ready Task can have no
live process; an absent terminal result cannot prove liveness. Binding to a done
Task assigns work history without reopening it.

```bash
lf comment INF-124 "keep the public name"
lf --wave product wave/operate "review the current priorities"
```

SQLite owns every Task comment. A save records the comment and its stable delivery
UUID in one transaction, before contacting Linear. CLI and Desktop read the thread,
pending IDs and conflict bodies from one SQLite snapshot. Mapped Tasks show pending sync;
a foreground Task Session or Desktop Task connection delivers saved comments and
acquires incoming comments independently. Lost replies are resolved by exact UUID,
issue and body. Incoming comments enter the same thread without echoing locally
saved direction. The shared skill path supplies Task context and live steers when
the checkout or explicit attribution selects a Task.
Idle steering starts no Flow. Prompt inclusion and provider acceptance do not
prove the model followed a correction. Wave planning uses ordinary finite
AgentSessions.

Work reservation sets Task Started once; an inspection Process does not. Constructors
validate Task/Wave ancestry, and chapter transfers preserve historical event
attribution. Task delivery separately owns Git/PR mutations. Conversation identity
and Flow membership never substitute for that authority.

[Delivery](delivery.md) follows Task Work through Git and GitHub.
[Execution](execution.md) owns process and conversation admission.

## Author and discover executable targets

`Target` is the shared executable value: Skill, Command, Flow, or Xor.
A `Flow` contains `Step { target, id, human, repeat }`; Xor paths contain the same Steps.
Loading resolves bare names and `flow:` names flow-first, including nested Flow
bodies, and rejects recursive references. `step:` selects a Skill explicitly,
retaining its name and options until compilation; Xor router names also load then.
Loading a Flow alone does not capture all instruction bodies.
There is no separate FlowRef node. Review IDs and repeat edges belong to the
Step itself, not to a reusable Target.

Compilation produces `ConcreteStep` plans containing every Skill body and Xor
router/path. The `sources` breadcrumb retains definition provenance for display.
These captured plans serve both Task and ordinary execution; a running Flow
never resolves names from current source.

Review Sessions launch the captured Skill explicitly, escaping command names.
Execution selection reads that captured value before looking for authored files;
help and list continue to inspect local definitions without Session state.
