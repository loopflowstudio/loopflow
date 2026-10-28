# Planning

```bash
lf --wave product wave-operate
lf task status INF-124 --json
lf checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
lf wave new-chapter --all 2026-10 --plan scratch/chapter.json --dry-run
```

Wave → Task is the navigation hierarchy. A Wave keeps its objective, memory,
cadence, budget and metric instruments across plans. Its one In Progress
Project owns Tasks, KRs and targets. A Chapter is the shared
name of those current Projects across the repository. The rotation below describes
the explicit shared Project binding. Rotation retains exact destinations and selected
issue IDs for recovery, while unreviewed backlog stays in its original Project.
[Cutover status](reference-cutover.md#cutover-status) records
other implementation and proof gaps.

## Planning authority and identity

```bash
lf task create --title "Fix the parser" --json
lf task comment <id> "Keep quoted input"
lf checkout <id>
```

Wave, Project and Task planning use one local SQLite owner with optional
repository-wide Linear synchronization. Creation without a Wave makes a standalone repository Task.
Wave goals, memory and other Markdown live only in `.lf/wave/<name>/` in the
checkout. Edit them like any other file; context and summaries read them directly.
Ancestor context uses the same checkout. Linear Initiative creation uses that
checkout; explicit planning sync reads GOAL.md at the committed local default-branch
tip, never unmerged branch edits or dirty working files. Default-branch selection
uses origin/HEAD (main when unset); sync does not fetch or move refs.
Both use the same 255-character description limit.
Sync writes only changed fields; plan-only sends nothing. A missing default-branch
ref or GOAL.md blocks mutation instead of clearing the provider summary.
Creation preserves authored IDs and bytes and writes missing GOAL.md/MEMORY.md
files. Reads never provision. Registry relocation changes addresses without moving authored files.
A Wave's default graph is a committed `.lf/graphs/` file or a builtin. A Wave
records only the selected name, which travels with planning.
Wave list, status and roadmap emit each Wave's `machine` from the reader's
Machine-local Wave → Machine default (the Wave, then its parents), or
`machine: null` when unset, never a fabricated local Machine.
Desktop consumes the same optional value.

Task deletion atomically records local removal and a stable pending field receipt.
It preserves Task graph, Session, Process, PR and checkout state, including completed
outcomes. Retry retains the first receipt. Confirmed historical provider removals
remain independent evidence. Active foreground connections deliver mapped removals.
An exact trash observation or deletion acknowledgement settles the receipt; missing
issues and lost replies retain uncertainty without replaying the mutation. A newer
explicitly active Linear revision retires removal and restores planning visibility,
retaining the losing receipt. This grants no execution or cleanup authority.

Task content is read directly from `Task` (identifier, title and description),
with its local write revision, optional Linear binding and observation age retained.
`Project` likewise owns its fields directly; neither has a separate Plan wrapper.
Project prose, KRs and metric targets are separate stored fields. Local
reads and the shared typed writer never parse or regenerate Markdown. Conversion
reads the released Markdown once inside the migration transaction, preserves
observation age, and seeds Git history only after conversion. Only the selected
current Project supplies the migrated Wave default graph; Project history
retains no graph authority.
Prompt assembly and Linear delivery render at their boundaries; malformed legacy
content refuses the migration rather than discarding criteria. Extraction and Linear
edits recognize fenced examples as prose and replace only owned field ranges;
unrecognized paragraphs survive even inside managed sections.
Project prose saves, Linear delivery and acquisition use the same typed writer.
Partial observations retain the prose baseline's own revision and age rather than
refreshing it with unrelated fields. Summary maps to Linear `description`; authored
prose belongs to Linear `content`. Managed fields and fenced examples survive replacement.

`ProjectKR` is an explicit judgment. `MetricTarget` names a metric and a numeric
`Threshold`, independently of the Wave's observations. Project iteration is no longer
live state; its stored values and historical judgment events remain readable evidence.
The typed field writer—not either complete domain struct—defines portable content;
flattening records does not make checkout or execution fields synchronizable.

`linear/` owns provider responses and acceptance; there is no generic PM facade.
`ops/planning_sync.rs` owns foreground exchange, store access and Wave locks for
both transports. Ordinary Task lookup and comments belong to `ops/task.rs`.
A Git lookup miss stays a Git lookup miss; it cannot fall through to Linear.
Unmapped local Projects remain local during Linear refresh until their export links them.

`work::task::comment::TaskComment` owns shared discussion. `TaskComments` joins
the thread with local delivery status. Linear comment IDs, author IDs and revisions
stay in adapter-internal observations and retained receipts; one adapter conversion
provides domain authorship. Local saves and acquisition share the typed comment writer.

Task and Project rows own identity for both paths. Locally authored Tasks mint a
UUID before placement and retain it through provider creation. New Linear acquisitions
derive the Task ID from the issue UUID; existing mapped Tasks keep their IDs.
Ticket aliases remain optional mappings. Repeated imports and ticket changes retain
identity. Unowned observations stay in provider evidence tables. Released-frontier
migration preserves existing identities, provider observations, PRs, Task graphs and
Session links. Provider content is retained as individual observation fields;
ordinary readers and ingestion never reconstruct a stored Project or issue object.
The membership index and provider claim remain separate when historical evidence
contradicts itself. Confirming refreshes compare the claim; receipt baselines retain
omitted versus explicitly null membership. Conversion refuses scalar type changes that SQLite would otherwise coerce,
retaining the original schema and input on failure.

`lf task create` generates the durable ID, saves the Task and returns that ID.
Each explicit invocation creates a distinct Task, including identical titles.
Internal synchronization retries reuse the saved identity. The transactional creation
receipt retains the original Project and input through later edits and rotation. The same transaction owns local field edits,
assignment, Project ordering and comments. Revisions are local write concurrency,
not fabricated provider observation times. Local Task selection displays the shortest
unique UUID prefix of at least seven digits; full IDs remain stable. References
accept four or more case-insensitive hex digits, bare or after `lf-`/`task_`;
ambiguous references return candidates instead of selecting a Task.

Saved Tasks use the same checkout preparation and placement transaction with or
without Linear. Checkout reads retained planning and branch metadata; it does not
refresh Linear. An unknown provider alias still needs initial acquisition.
Acquisition returns the saved Task. Its freshness and ownership lookup reads only
retained Linear identity, revision, membership, invalidation, removal and age;
it never reconstructs provider content. A delayed absent response invalidates only
the exact issue revision and observation age it queried.
Preparation uses the saved Task identity and title directly. New branch names retain
the full Task UUID; default checkout names use up to four title words and an ID
suffix. Retained provider branch names take precedence.
Placement records the checkout before filesystem creation, without creating a PR or rewriting
planning fields. Accepted state and completion are retained on the Task, so terminal
planning prevents first placement even without provider inventory. Task and Graph entry
use the same SQLite admission reader as placement, without a provider resolver.
It reads one snapshot of saved planning, Task graph state and retained invalidation,
removal and membership evidence. Missing provider inventory alone does not prevent
a saved Task from running; a retained invalidation still does. Remote completion
prevents new work but does not stop an already running Task. Incoming removal
retains the Task and marks its planning deleted; neither observation moves a Task
or cleans a checkout. Competing reservations return the saved allocation; restoration
uses that checkout and its optional PR. The queued writer retains the Wave lock through commit,
even if its caller is canceled. Native launch and resume retain the
ordinary Session and Process authority checks. Local deletion receipts and retained
provider deletion evidence apply regardless of planning mode. Local completion
records its planning decision and optional delivery receipt without moving the Task's graph.
GitHub delivery still requires confirmed merge evidence. Chapter rotation commits
all selected Waves together, carrying started Tasks and preserving backlog. Stable
local IDs select the rows and retry receipts; provider aliases select existing
Projects through their stored mapping. Membership, Project fields, selection and
pending effects commit together. Connected commands report a pending planning sync.
Inbound observations preserve pending membership against an unchanged baseline;
conflicting Linear membership wins while the losing local intention remains in history.
Mapped membership and Project fields use foreground delivery below; local settlement
alone does not acknowledge an external effect.

Refiling resolves the saved Task and destination Project, holds both Wave locks,
and commits membership and its field receipt together. Existing work, including
Sessions, Processes, Graphs and PRs, retains its owner. Retrying the selected
destination changes neither the revision nor receipt. Pending refiling allows inbound
observations from another Wave to adopt conflicting Linear membership, retain both
values and advance unrelated fields. It never grants execution or provider-write authority.

One typed writer, `sqlite/planning_write.rs`, owns planning field persistence and
Git mutation capture. Local writes and journal mutations carry `WorkEdit`: a target
identity and a typed `WorkValue`. The value determines its scope and conflict field;
Git and the local journal retain the same typed mutation, including its causal
parents. SQLite derives target, field and clock indexes from that value; its table
routing stays private to the writer. Import uses those values
directly, with the same record construction as local writes. The caller's transaction includes admission, planning,
local delivery receipts and mutation identity. Execution columns are absent from
this writer. Local creation attaches machine-specific metadata afterward in the
same transaction. Import never creates execution placement or provider attempts.

Repository configuration selects Linear, shared Git (`shared/main` by default) or
personal Git.
Repository `planning` overrides the personal setting as one value. Linear-connected
machines acquire and deliver independently through their existing local receipts.
An outage buffers changes locally; Git cannot take over. Git transports code in
both modes. Its planning document contains no Linear observations, attempt receipts,
creation recovery or receipt-settlement protocol. No designated publisher exists.

Git imports retain immutable mutations before projection. Field causality precedes
logical clocks and stable mutation-ID ties. All losing values remain inspectable.
`planning_peer_observed` records only successfully applied heads: a rejected import
cannot become the causal parent of a later local save. Projection uses per-object
savepoints so a conflicting parent preserves that object's history while independent
objects advance. Wave selection follows Project projection. Invalid documents,
reused mutation IDs and foreign repository ownership roll back the import. Repeated
imports create no echo edits. Fetch, import and publication have distinct receipts;
a lost push reply requires readback before publication is reported confirmed.

Current Work ownership selects new Git exports. Each exported or imported mutation
retains its repository and destination in `planning_peer_routes`, independently of
live Work rows. Switching refs, moving Work or deleting a row cannot erase earlier
routing or rejected imports. Status derives the active destination from configuration;
there is no stored selection or per-Work membership inventory. These routes are
exchange evidence and grant no Task ownership. Ownership entries travel as marked
Task comments in both modes; with Git planning, each local append sorts after every
entry its Machine has seen.

Task and Project Linear field receipts share `sqlite/planning_changes.rs`, with
separate foreign keys. They retain stable mutation identity, provider baseline and
conflicting values. An unchanged baseline preserves pending local edits; an observed
conflict adopts Linear and retains the losing intention. `planning_order.rs` saves
one Project order receipt and neighboring Task ranks in the same transaction.
Complete-list evidence owns ordering; detail reads never reorder Tasks. Uncertain
writes remain attempted and require provider readback before another effect.

Creation captures a stable provider UUID and exact input before network I/O.
Task creation reads canonical Work and retains only shared creation-field baselines,
not a provider-shaped Task copy or local execution data. Existing receipts keep their
original input and baseline bytes through retries.
Acquisition attaches the provider mapping to the original local row. Unknown creation
or attachment outcomes retain their receipts; a matching title proves nothing.
Comments retain identity and authorship. Incoming planning completion changes only
the planning disposition: it neither moves a local Task nor authorizes cleanup.
Session/Process records and checkout paths never enter the Git document.

`PlanningSyncStatus` presents local Linear delivery evidence; `lf planning status`
presents Git acquisition/publication receipts and held projections. The retained
mutation journal grows with every edit, so `lf planning journal` reads it on
request: every change, losing values included, and the ranked candidate IDs, which
are not applied or published evidence; rejected projections retain their failure reasons.
Journal read failures fail the status read; Desktop keeps its last reading with the
error, rather than inventing an empty plan. Foreground CLI, headless Session and Desktop connections own Git exchange
for their lifetimes; Linear checks run at the boundaries described below. A short save attempts exchange before returning. No resident,
whole-store callback, scheduler or implicit turn/Graph replay is involved.

`--machine` sends portable pushed-code requirements and a Task selector, then the
target resolves its own configured planning. Linear mappings resolve independently
retained legacy IDs. Git imports preserve local planning IDs. A source Task awaiting
Linear creation is unavailable remotely until publication succeeds. Fetch precedes
code decisions, including branches without PRs. Repeated dispatch reuses the existing
Task and checkout; unpushed source work and a target missing the requested commit
produce actionable errors without committing, resetting or replacing local work.

The personal namespace and provider-first planning writers are deleted. Connected
CLI and Desktop project creation, field/order edits, state and comments through
`PlanningSyncStatus`, derived from their existing receipts. Unmapped creation is
pending in connected repositories; attempted effects retain uncertainty and errors.
Observed conflicts show both values after adopting Linear. Disconnected repositories
show no pending Linear delivery; retained losing values remain inspectable.
Planning acquisition runs at useful boundaries: before Graph steps, after agent
turns, before Linear mutations, and when CLI commands show planning. One team
query obtains issues and comments changed since its successful watermark; large
deltas paginate without advancing that watermark early. SQLite shares a ten-second
attempt stamp per team across repositories and processes; an acquisition lock
covers the short request/ingestion operation, with no resident owner or hand-off.
Failed checks keep the prior watermark and local planning.

Teams without live Tasks are not checked. Locally finished/abandoned identities
are excluded from acquisition. Explicit detail reads remain available. Desktop and
`--watch` alone own a clock: 30–60 seconds initially, backing off to four minutes
while unchanged. Closing the watcher stops it; idle agents own no Linear timer. Git exchange retains its foreground lifetime.
Projection functions remain local; command boundaries acquire before projecting.

Linear HTTP 400 rate-limit errors, 429 and exhausted request/endpoint/complexity
headers establish a Home-wide pause across repositories and refreshed logins.
Reset headers are epoch milliseconds; missing deadlines fall back to one hour.
In-flight requests may finish, and a successful final response remains usable.
One warning announces the window; later calls return its deadline without network
access. After reset, the next boundary retries pending identities. Successful
mutation payloads supply confirmation without a follow-up read. Lost responses
retain uncertainty; positive rate-limit rejection makes that receipt retryable.
Independent Homes do not share this pause.
Ingestion adopts observed Linear conflicts and retains the losing
local intentions in field, state and comment receipts. Retired intentions never
reenter delivery after a late acknowledgement or matching observation.
The documented unconditional-update race remains a protocol limit.

## Project creation and selection

```bash
lf wave bind-project product <project-uuid> --json
lf wave ensure product --json
```

Validate one existing Project and select its accepted local record on the Wave's
SQLite row (`waves.current_project_id`). Repeating the same binding preserves it;
a different existing binding is left unchanged. Setup preserves provider status,
content and identity, including a Backlog Project. It does not
activate the Project. Operation routing and status select this exact ID; the JSON
Project summary carries a required `current` boolean for Desktop. Status and
roadmap retain predecessor Projects and Tasks. Registration and unstarted managed
work require that exact Project to be In Progress; another In Progress Project
does not compete with the binding. Already-started Tasks retain continuation in
predecessor Projects.

Wave and roadmap Task summaries read saved Work directly: `id` is the local Task
identity, `linear_id` is its optional provider binding, and `sync` is required
read-only evidence. Missing provider inventory does not erase saved prose, due
dates or assignments. Graph, execution and checkout evidence remain separate.

`ensure` activates the configured Backlog or Planned Project in SQLite and records
its status change for delivery. Without a binding, creation, original name,
selection and the local transition receipt commit together. Retry reuses that
identity and preserves later edits. Existing provider creation reservations retain
their UUID and mappings. No provider request is needed, including during an outage.
Terminal, archived, paused or foreign Projects retain their history. Names and content remain intact; the Wave owns its default graph. Binding accepts the durable or mapped ID of
an already saved Project; it does not fetch a missing Project. Legacy YAML import
retains the original bytes and requires that exact Project's saved record.
Project export uses the original transition receipt on the foreground connection.
Creation and Initiative attachment retain separate attempted effects and exact
mutation acknowledgements; a missing response never permits another create or attachment.
Mapped status receipts use foreground delivery;
a locally active Project alone is not proof of Linear activation. Ensure never searches for candidates or rotates
Projects; status and roadmap never call it. Desktop
ensures on explicit opening or retry while retaining cached planning and independent
Session reads. Both primary and Portfolio surfaces render the same SQLite-derived
`project_readiness` through `lf wave show --watch --json`. Committed selection,
accepted facts, transitions and exact activation Process outcomes invalidate that reading.
A pending transition or unfinished Process is unresolved evidence, not proof of liveness.
Watchers and status reads never provision or import.

The first explicit ensure, binding setup or applied rotation imports an existing
`<Machine>/waves/<WaveId>/config.yaml` selection once. Binding and import resolve
exact durable or mapped IDs in the same SQLite transaction as selection; names and
slugs never select a Project. SQLite commits the selected Project and original YAML
bytes together in `project_binding_imports`; absent files are recorded too. Malformed
files, missing identities or a conflicting selection leave import retryable and
prohibit creation. After import, the file
is inert, even if a stale checkout or old writer changes it. Other YAML bytes remain
untouched. An imported completed Project stays selected history and cannot be reopened.
Activation records its exact Process on the Wave; that Process remains the sole owner of
its terminal outcome and error. Swift retains command transport feedback only.

## Rotate the plan, preserve the work

```bash
lf wave new-chapter --all 2026-10 --plan scratch/chapter.json --dry-run --json
lf wave new-chapter --all 2026-10 --plan scratch/chapter.json --json
lf wave new-chapter product 2026-10 --plan scratch/chapter.json --json
```

Rotation previews and results return canonical Tasks with local IDs and explicit
Linear bindings. Classification reads their saved disposition and independent local
start evidence; it does not reconstruct provider records or resolve the Task twice.

Retain one JSON input with `name` and `waves`. Each entry supplies `wave_id`,
`successor_id`, `create`, `project_name`, `metric_targets` and `krs`:

```json
{
  "name": "2026-10",
  "waves": [{
    "wave_id": "<registered-wave-id>",
    "successor_id": "<new-or-existing-project-uuid>",
    "create": true,
    "project_name": "Autumn customer work",
    "metric_targets": [],
    "krs": [{"text": "Customers can resume unfinished work", "holds": false}]
  }]
}
```

Allocate a new UUID once when authoring a creation plan. Set `create` to false
for an existing exact destination; absence never turns that instruction into
creation. Every destination requires authored KRs before any provider write.
Existing Projects keep their names, summaries and unrelated content; supplied
KRs and targets replace only those planning fields. Rotation preserves the Wave
default graph.

The repository operation validates all selected Waves, destinations, legacy
conversions and Task evidence before reserving every pair and applying the first
Wave. The Wave command consumes only its entry through the same operation.
The shared binding identifies the predecessor. Names never select either endpoint.
Unfinished Tasks with any ownership entry move with identity, checkout, PR and
captured execution intact. Unreviewed backlog remains historical; unknown evidence blocks apply.

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
lf task status INF-124 --json      # the Task row, Sessions, Graph processes, comments
lf desktop task INF-124 --json     # the planning evidence below
```

`lf desktop task` returns canonical `task` and `project` records, `planning_state`,
`observed_at`, `planning_error`, `planning_stale`, delivery `sync` and optional
`execution`. Local IDs and explicit Linear bindings stay distinct. A foreign or
Project-less observation returns acquisition evidence without fabricating owned
Work; its concrete record remains retained internally. Status may acquire planning
but never completes a Task, allocates a checkout or moves a Task's graph.
Wave, roadmap and Work frames expose the same acquisition state and age separately
from saved content and outbound delivery.

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
`observed_at` remains the last successful acquisition. An unresolved
selector returns an unavailable envelope even without execution.

Invalid and removed facts remain inspectable and withdraw new-launch availability,
without settling or stopping existing execution. Saved Work supports offline
admission; observation age and missing inventory alone do not erase it. Wave
`synced_at` dates its last successful list; entities may have newer detail evidence.

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

Private Linear inventory/detail acquisition commits retained observations and
Projects/Tasks in the same SQLite transaction. Public Store APIs expose canonical
Work and narrow acquisition evidence, never fetched response models. Ordering
receipts receive accepted local Task IDs; new disposition conflict receipts retain
revision/state/completion evidence. Historical receipts keep their original bytes. Projection uses each stored entity's
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
predecessor Project. The same local transaction marks the predecessor completed,
selects the successor and retains each pending provider field change. It changes
neither Task execution nor historical KR results. Retry consumes the original plan
and selected membership, preserving later edits. Pre-cutover unfinished provider
transitions remain unresolved with their original receipts; local rotation does not
claim to recover their uncertain external effects.

Historical provider transitions and archival acknowledgements remain evidence;
local rotation does not recreate the removed provider-first chapter operation
or infer that its uncertain effects settled.

## Capture a Graph once

```yaml
# .lf/graphs/build.yaml
- step: implement
- step: compress
- step: gate
```

```bash
lf graph build
lf graph show GRAPH_PROCESS --processes --json
```

A Graph is one lf process and the step processes it starts; its ID is the Graph
process's. Starting one compiles its definition, including every router and
alternative, into a graph the Graph process holds in memory. One cursor and its return
counters identify loop passes. Subgraphs and passes are display lenses over the
step processes, with no separate record or process.

Every Graph naming a Task, or run in its checkout, is equally that Task's
work; none is selected or privileged. Taskless and Task execution share the
Graph runner. `lf graph` runs an `interactive:` step in place, with the person at a
terminal, else headless. Each `task run` starts a fresh Graph as a child Graph process
(hidden `lf --task ISSUE graph run`), and a fresh one again when that Graph process
fails; it stops before the next interactive step (`GraphOutcome::Staged`). Finishing
retains history and chooses no successor; Graph completion alone does not
complete Task Work.

## Carry a Task through its graph

A Task takes up [one graph](../orchestration.md#where-a-person-takes-part) and stands
at a step of it. On its first `lf task run`, a Task takes up the graph named on
`lf task run ISSUE GRAPH`, else its Wave's default, repository `graph:`, then
builtin `code`. `task_graphs` holds one row per Task: the graph as it was then,
never edited, and `step`: `start`, `end`, or the interactive step it waits at.
A later edit to the YAML applies to Tasks that take it up afterwards; taking one
up again replaces the row and starts over at `start`.

While `lf task run` carries the Task, the row also holds `run_start`, the cursor
that run began from, and `lf_process_id`, that Task run. Whether it still runs is
read from the Process, never stored. The run's child Graph process writes where it
left the Task: before the interactive step it stopped at, or `end`. A run that
stops leaves the Task on it; the next `lf task run` starts it again from
`run_start`. Without one, `lf task run` continues after the step the Task waits
at; `--from STEP` (a step id, skill or composed graph name) starts anywhere, and
starting from an interactive step only puts the Task there. `-b task run` runs
straight through headless, and `lf ship` repeats it until `end`. Other Graphs in
the checkout never move the Task. Graph processes and Task events keep the
history; there is no move log.

Task completion, graph position and Process liveness are independent.
`task complete` changes status without moving or creating a Task graph. Reaching
`end` atomically records the position and a completion request, then
attempts settlement. Failed settlement retains end and its reason; retry only
`task complete`, never the successful run. PR delivery requires verified merge; PR-less completion retains useful artifacts.
Accepted Linear completion or reopening updates status without moving the graph
or terminating Processes. Reopening supersedes old requests. Ordinary reads do
not execute completion. Task state is derived: `active` at an interactive step
with nothing running, `moving` while a run carries it with an interactive step
ahead, `shipping` once Auto-Ship is approved and running; abandonment is its own mark.

On-demand team checks acquire issue membership/state independently of
comment and state delivery. Delivery receipts retain their original provider revision,
attempt evidence and conflicting value. The latest receipt determines the displayed
writeback state; Tasks store no separate copy. A response records only its captured
effect, so it cannot settle a newer decision. State acquisition and delivery share
one transactional reconciliation rule: observed conflicts adopt Linear and settle
the losing receipt without moving the Task. No local/provider clock comparison orders edits.
The provider read and mutation remain separate requests; they do not prevent a
concurrent Linear edit between them. A matching mutation response settles the observed target;
it does not prove that no intermediate edit was overwritten. An unseen complete/reopen
cycle can disappear under the unconditional write, leaving no observed conflict
to retain. Another read cannot close that race.

Jack Heart's October 8 conflict decision and subsequent regression-repair request
replace the former test's absolute unseen-reopening protection and pending-loser
expectations. Enabled operation tests now distinguish reopening observed before
delivery or in the mutation response (Linear wins, both values retained, loser retired, no
execution changes) from the original unseen interleaving (overwrite remains
possible). Lost replies retain attempt identity and reconcile without replay.
This is an explicit concurrency limit, not a provider atomicity guarantee.

Abandonment saves the decision and cancellation
receipt together without provider access. Cleanup failures preserve the decision;
unknown or live execution prevents cleanup before provider inspection. Cancellation
uses the same state delivery path as completion and reopening. Resolve the issue
team and target state before marking a receipt attempted, so failed reads remain
retryable. A lost mutation reply retains uncertainty until provider observation.
Delivery at save, check and watcher boundaries selects unmapped local Projects and Tasks,
using their saved UUIDs. Project transitions and Task creation receipts retain the
captured payload, attempted effects and errors; no second creation owner exists.
Exact observations attach mappings in the common ingestion transaction before
inventory can allocate another local identity. The creation snapshot establishes
field baselines without acknowledging later edits. Observed Linear conflicts still
win and retain the losing receipt. Removed Tasks export only when an attempted
creation needs reconciliation, then their existing deletion receipt owns removal.
Creation receipt changes advance the planning revision, so active readers acquire
new attempt errors and confirmations. Composed CLI/Desktop reconnect remains unproved.

## Read each step's result

Each executed skill or operation runs in its own child lf Process as the plain
command: `lf -b skill <name> [message]`, or the operation's own command. The
step is told nothing about its Graph. The Graph process owns navigation and the Graph's
record, GraphProcess: the Graph's name and graph as compiled at launch, then one
appended row per step with its Process, graph node key and per-edge iterations.
Nothing reads it back to resume. A step's result is how its process exited; a
deciding or routing step also answers through the final answer of the Session
turn its Process captured. After an operation the Graph process stops the Graph when a
landing of its checkout is still being watched: neither failed.

A deciding step's message asks for a JSON `decision`: `advance` or `iterate`
with a nonempty `summary`, or `blocked` with a nonempty `reason`; the unused
field is null. A router's message asks for a JSON object containing `path`, one
of the branch's path names. The contract travels in the message, not as a
provider schema, and the Graph process accepts the value inside prose or a code fence.
Session history retains the native output and completion separately. An invalid
answer is corrected by `lf -b connect ID --message TEXT` in the same
conversation, at most twice, then the Graph fails; provider failure remains
distinct.

Blocked records the reason and stops at the current Graph position. Existing logs
and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. Nothing retries
or resumes a stopped Graph.

## Inspect before further work

```bash
lf task status INF-124
lf graph show GRAPH_PROCESS --processes --json
lf task interrupt INF-124
lf task run INF-124 --reason "take the smaller approach"
```

A killed Graph process leaves its Processes and GraphProcess rows as history; the last step
row shows where it stood. Nothing restarts or resumes it. A Graph is `current` while its
Graph process has no recorded exit, `completed` when it succeeded and `stopped`
when it exited before the last step. Crash recovery
belongs to the caller, normally the Task conversation: inspect the Session,
process and effect receipts, then launch fresh work. Observation does not replay
work or consume a surviving child's result.

`task run` places the Task, then runs a fresh ordinary Graph in its checkout:
the same command, checks and records as `lf --task ISSUE graph GRAPH` or a run
from the worktree. It blocks until the Graph ends; a caller that will not wait
backgrounds it. `--reason` publishes direction to the Task first. To change
direction, interrupt, inspect, then run.

Task status and roadmap `execution` observe the Task's most recently launched
Graph: `none`, `latest` or `finished`. Its only control is `start`, available
whenever the Task can run, even when an earlier Graph exists.

Automatic provider retries within one Process retain earlier failure and usage and
select only an authorized successor. An uncertain mechanical effect requires
inspection; moving a cursor does not establish exactly-once external effects.
Missing process evidence is uncertainty, and causal ancestry grants no signal
authority.

## Task conversations

```bash
lf session list --json
lf connect SESSION --prepare --json
```

Discuss design and review feedback in the ongoing Task conversation. Historical
review feedback remains Session evidence. No Ready/Complete operation closes the
conversation or releases a Graph. The caller inspects outcomes and effects
before selecting further work; pane closure and provider exit grant no authority.

Saved handoffs retain the Machine and stable installed entry; private-Home
handoffs retain their explicit executable. Renaming, binding and takeover retain
conversation identity and feedback. Desktop reads the same record and keys its
terminal surface on LfSession identity.

## Work, steering and execution

Task status is `Ready`, `Done` or `Abandoned`. Current activity, command outcome,
conversation state and Graph progress remain separate. A ready Task can have no
live process; an absent terminal result cannot prove liveness. Binding to a done
Task assigns work history without reopening it.

```bash
lf comment INF-124 "keep the public name"
lf --wave product wave-operate "recover the stopped release Task"
```

SQLite owns every Task comment. A save records the comment and its stable delivery
UUID in one transaction, before contacting Linear. CLI and Desktop read the thread,
pending IDs and losing local bodies from one SQLite snapshot. An observed comment
conflict adopts Linear’s body, author and time while the receipt retains both
complete comments; no replacement comment is created. Connected repositories show
pending sync, including comments on unmapped Tasks. Saves and on-demand boundaries
attempt delivery; team checks acquire incoming comments independently. Desktop and
`--watch` continue checks while open. Lost replies are resolved by exact UUID,
issue and body. Incoming comments enter the same thread without echoing locally
saved steers. Comments and adopted definition revisions save immutable
steers. Each Session selects steers from checkout/explicit Task associations, Wave
ancestors and repository. Provider callbacks deliver bounded batches at startup,
continuation and tool boundaries; the Task brief reloads independently of consumed
steers. Idle steering starts no Graph. Local emission
remains unconfirmed and does not prove the model followed a correction. Wave
planning uses ordinary finite LfSessions.

Work reservation claims an unowned Task in its ownership log; an inspection Process does not. Constructors
validate Task/Wave ancestry, and chapter transfers preserve historical event
attribution. Task delivery separately owns Git/PR mutations. Conversation identity
and Graph membership never substitute for that authority.

[Delivery](delivery.md) follows Task Work through Git and GitHub.
[Execution](execution.md) owns process and conversation admission.

## Author and discover executable targets

`Target` is the shared executable value: Skill, Command, Graph, or Xor.
A `Graph` contains `Step { target, id, interactive, returns }`; Xor paths contain the same Steps.
Loading resolves bare names and `graph:` names graph-first, including nested Graph
bodies, and rejects recursive references. `step:` selects a Skill explicitly,
retaining its name and options until compilation; Xor router names also load then.
Loading a Graph alone does not capture all instruction bodies.
There is no separate GraphRef node. Occurrence IDs and return targets belong to the
Step itself, not to a reusable Target.

Compilation produces `ConcreteStep` plans containing every Skill body and Xor
router/path. The `sources` breadcrumb retains definition provenance for display.
These captured plans serve both Task and ordinary execution; a running Graph
never resolves names from current source.

Review Sessions launch the captured Skill explicitly, escaping command names.
Execution selection reads that captured value before looking for authored files;
help and list continue to inspect local definitions without Session state.
