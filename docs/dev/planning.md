# Planning

```bash
lf --wave product wave-operate
lf task status INF-124 --json
lf checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
lf wave update-project --wave product --file scratch/product-project.json
```

Wave → Task is the navigation hierarchy. A Wave keeps its objective, memory,
cadence, budget and metric instruments across Projects. Its current Project
owns Tasks, KRs and targets; connected to Linear, it is one In Progress Linear
Project. A Wave's Project is edited in place and `lf wave plan bind`
selects another existing Linear Project.
[Cutover status](reference-cutover.md#cutover-status) records
other implementation and proof gaps.

## Words

| Word | Means | Not |
|---|---|---|
| plan | the shared Waves, Projects and Tasks | |
| Project | a Wave's current Project: its KRs, targets and Tasks | |
| tracker | where the plan is mirrored: Linear or Asana; `tracker:` in `.lf/config.yaml`, shown by `lf repo tracker` | bridge, planning provider; provider means the agent's provider |
| stamp | `(at, machine id)` on one planning field; the newest stamp is the plan | a revision, a clock comparison with a tracker |
| carrier | what moves stamped records between Machines: Git plan refs, a hub | a tracker; a carrier decides nothing |
| hub | a Machine others exchange stamped records with over the server connection; `lf machine hub` | the plan's home, a tracker |
| unsent | a record changed here that a carrier has not taken yet; counted per carrier | open, queued |
| action | one outgoing change for Linear or Asana in `tracker_actions`, never for a carrier: queued or sent (open), then confirmed, conflicted (lost, until answered) or retired. Prose near Desktop says "tracker action": Desktop's actions are buttons | effect, receipt, delivery, export |
| event | one incoming change from a tracker | observation, acceptance, acquisition |
| sync | moving the plan in either direction | |
| Work | the model's name, in dev docs only | |

## Planning authority and identity

```bash
lf task create --title "Fix the parser" --json
lf steer <id> "Keep quoted input"
lf checkout <id>
```

Wave and Task planning use one local SQLite owner with optional
repository-wide Linear synchronization. Creation without a Wave makes a standalone repository Task.
Wave goals, memory and other Markdown live only in `.lf/wave/<name>/` in the
checkout. Edit them like any other file; context and summaries read them directly.
Ancestor context uses the same checkout. Linear Initiative creation uses that
checkout; explicit planning sync reads GOAL.md at the committed local default-branch
tip, never unmerged branch edits or dirty working files. Default-branch selection
uses origin/HEAD (main when unset); sync does not fetch or move refs.
Both use the same 255-character description limit.
Sync writes only changed fields; plan-only sends nothing. A missing default-branch
ref or GOAL.md blocks mutation instead of clearing the tracker summary.
Creation preserves authored IDs and bytes and writes missing GOAL.md/MEMORY.md
files. Reads never provision. Registry relocation changes addresses without moving authored files.
`wave_lifecycle_operations` retains Machine-local rename/removal effects: original
Wave/subtree IDs, captured tracker bindings, and exact pending authored changes.
The lifecycle saves intent before planning edits, prepares file changes off-checkout,
and commits saved after-images through a private index. Retries retain unfinished
effects, exclude replacement identities and preserve later edits. These receipts
never travel to another Machine; Wave identity and retirement remain
planning-writer facts.
A Wave's default graph is a committed `.lf/graphs/` file or a builtin. A Wave
records only the selected name, which travels with planning.
Wave list, status and `plan list` emit each Wave's `machine` from the reader's
Machine-local Wave → Machine default (the Wave, then its parents), or
`machine: null` when unset, never a fabricated local Machine.
Desktop consumes the same optional value.

Task deletion atomically records local removal and a stable pending field action.
It preserves Task graph, Session, Process, PR and checkout state, including completed
outcomes. Retry retains the first action. Confirmed historical tracker removals
remain independent evidence. Active foreground connections deliver mapped removals.
An exact trash event or deletion acknowledgement settles the action; missing
issues and lost replies retain uncertainty without replaying the mutation. A newer
explicitly active Linear revision retires removal and restores planning visibility,
retaining the losing action. This grants no execution or cleanup authority.

Task content is read directly from `Task` (identifier, title and description),
with its local write revision, optional tracker binding and last refresh time.
`Project` likewise owns its fields directly; neither has a separate Plan wrapper.
Project prose, KRs and metric targets are separate stored fields. Local
reads and the shared typed writer never parse or regenerate Markdown. Conversion
reads the released Markdown once inside the migration transaction, preserves
the last refresh time. Only the selected
current Project supplies the migrated Wave default graph; Project history
retains no graph authority.
Prompt assembly and Linear delivery render at their boundaries; malformed legacy
content refuses the migration rather than discarding criteria. Extraction and Linear
edits recognize fenced examples as prose and replace only owned field ranges;
unrecognized paragraphs survive even inside managed sections.
Project prose saves, Linear delivery and refresh use the same typed writer.
Partial events retain the prose baseline's own revision and age rather than
refreshing it with unrelated fields. Summary maps to Linear `description`; authored
prose belongs to Linear `content`. Managed fields and fenced examples survive replacement.

`ProjectKR` is an explicit judgment. `MetricTarget` names a metric and a numeric
`Threshold`, independently of the Wave's observations. Project iteration is no longer
live state; its stored values and historical judgment events remain readable evidence.
The typed field writer—not either complete domain struct—defines portable content;
flattening records does not make checkout or execution fields synchronizable.

`ops/tracker.rs` selects the repository's tracker, `Tracker { Linear, Asana }`,
from `.lf/config.yaml`; `linear/` and `asana/` own tracker requests, responses and
incoming events. The store never builds tracker input, and no `Linear*` type leaves
`linear/`: trackers fail with `tracker::TrackerError`.
`ops/planning_sync.rs` owns save delivery, store access and Wave locks;
`ops/plan_hub.rs` owns the exchange with a hub, `ops/plan_git.rs` the one with
a remote, and `ops/plan_carriers.rs` when the two run together. Ordinary Task
lookup and comments belong to `ops/task.rs`. A lookup miss without a tracker
exchanges once by each carrier; it cannot fall through to Linear.
Unmapped local Projects remain local during Linear refresh until their export links them.

`loopflow_model::comment::TaskComment` owns shared discussion. `TaskComments` joins
the thread with local delivery status. Linear comment IDs, author IDs and revisions
stay in tracker events and actions; one adapter conversion
provides domain authorship. Local saves and refresh share the typed comment writer.

Task and Project rows own identity for both paths. Locally authored Tasks mint a
UUID before placement and retain it through tracker creation. New Tasks imported from Linear
derive the Task ID from the issue UUID; existing mapped Tasks keep their IDs.
Ticket aliases remain optional mappings. Repeated imports and ticket changes retain
identity. Unowned events stay in tracker evidence tables. Released-frontier
migration preserves existing identities, tracker events, PRs, Task graphs and
Session links. The store retains tracker content in individual fields;
ordinary readers and ingestion never reconstruct a stored Project or issue object.
The membership index and tracker claim remain separate when historical evidence
contradicts itself. Confirming refreshes compare the claim; action baselines retain
omitted versus explicitly null membership. Conversion refuses scalar type changes that SQLite would otherwise coerce,
retaining the original schema and input on failure.

`lf task create` saves the Task and returns a creation result containing `task`,
`graph`, `session_id` and `milestone`. A fresh invocation generates a request ID;
`--request-id` supplies one for safe recovery. Identical content with the same ID
reuses the original creation, while different content conflicts. A fresh ID means
a new submission, even with an identical title. The shared creation receipt
retains the original identity through retries; tracker synchronization separately
retains the original Project and input through later edits. The same transaction owns local field edits,
assignment, Project ordering and comments. Revisions are local write concurrency,
not fabricated tracker event times. Local Task selection displays the shortest
unique UUID prefix of at least seven digits; full IDs remain stable. References
accept four or more case-insensitive hex digits, bare or after `lf-`/`task_`;
ambiguous references return candidates instead of selecting a Task.

Saved Tasks use the same checkout preparation and placement transaction with or
without Linear. Checkout reads retained planning and branch metadata; it does not
refresh Linear. An unknown tracker alias still needs an initial refresh.
Refresh returns the saved Task. Its freshness and ownership lookup reads only
retained Linear identity, revision, membership, invalidation, removal and age;
it never reconstructs tracker content. A delayed absent response invalidates only
the exact issue revision and refresh timestamp it queried.
Preparation uses the saved Task identity and title directly. New branch names retain
the full Task UUID; default checkout names use up to four title words and an ID
suffix. Retained tracker branch names take precedence.
Placement records the checkout before filesystem creation, without creating a PR or rewriting
planning fields. Accepted state and completion are retained on the Task, so terminal
planning prevents first placement even without tracker inventory. Task and Graph entry
use the same SQLite admission reader as placement, without a tracker resolver.
It reads one snapshot of saved planning, Task graph state and retained invalidation,
removal and membership evidence. Missing tracker inventory alone does not prevent
a saved Task from running; a retained invalidation still does. Remote completion
prevents new work but does not stop an already running Task. Incoming removal
retains the Task and marks its planning deleted; neither event moves a Task
or cleans a checkout. Competing reservations return the saved allocation; restoration
uses that checkout and its optional PR. The queued writer retains the Wave lock through commit,
even if its caller is canceled. Native launch and resume retain the
ordinary Session and Process authority checks. Local deletion actions and retained
tracker deletion evidence apply regardless of planning mode. Local completion
records its planning decision and optional tracker action without moving the Task's graph.
GitHub delivery still requires confirmed merge evidence. Stable
local IDs select the rows and retry actions; tracker aliases select existing
Projects through their stored mapping. Membership, Project fields, selection and
pending effects commit together. Connected commands report a pending planning sync.
Inbound events preserve pending membership against an unchanged baseline;
conflicting Linear membership wins while the losing local intention remains in history.
Mapped membership and Project fields use foreground delivery below; local settlement
alone does not acknowledge an external effect.

Refiling resolves the saved Task and destination Project, holds both Wave locks,
and commits membership and its field action together. Existing work, including
Sessions, Processes, Graphs and PRs, retains its owner. Retrying the selected
destination changes neither the revision nor action. Pending refiling allows inbound
events from another Wave to adopt conflicting Linear membership, retain both
values and advance unrelated fields. It never grants execution or tracker-write authority.

One typed writer, `sqlite/planning_write.rs`, owns planning field persistence.
Local writes, tracker events and records arriving from another Machine all carry `WorkEdit`: a target
identity and a typed `WorkValue`. The value determines its scope and conflict
field; table routing stays private to the writer. Imports use those values
directly, with the same record construction as local writes. The caller's
transaction includes admission, planning, local delivery actions and the owner's
change stamp. Execution columns are absent from this writer. Local creation
attaches machine-specific metadata afterward in the same transaction. Import
never creates execution placement or tracker attempts.

Repository configuration selects Linear, Asana or neither. Repository `tracker`
overrides the personal setting as one value; the released `provider: git` reads
as neither. Linear-connected Machines acquire and deliver independently through
their own actions; an outage buffers changes locally.

### Without a tracker: stamps and carriers

```text
laptop$ lf task edit 3e1c9a --title one      # stamped (1763474520113, laptop)
mini$   lf task edit 3e1c9a --title two      # stamped (1763474529870, mini)
# after any exchange, in any order, by any carrier: "two" on both
```

Without a tracker, each Machine's store holds the whole plan and one stamp per
field (`plan_stamps`). The plan a Machine shows is, for each field, the value
with the newest stamp it has received. Nothing else decides: there is no
conflict state between Loopflow Machines and no Machine owns the plan.

- **Stamp.** `(at, machine id)`, compared in that order. `at` is
  `max(wall clock in ms, highest at this Machine has written or seen + 1)`, so
  an edit made after seeing yours beats yours whatever either clock says, and
  edits that never saw each other resolve by when they were made. A save stamps
  every field it changes inside its own transaction. A field with no stamp row
  reads as `(0, this Machine)`: any save beats it.
- **Record.** A record's id, its table and every field it has, each with
  `{value, at, machine}`. `plan.apply`, `plan.changes` and a file in a plan ref
  carry exactly this, from one serializer.
- **One way in.** `planning_write` takes stamped records from elsewhere through
  one function: each field is written (`WriteOrigin::Import`) only when its
  stamp is newer than the one held, and the clock moves past the highest seen.
  Applying a record twice, late or from two carriers gives the same plan.
- **`plan_changes`** is the local order in which records changed. A local save
  bumps its record; so does an arriving value that wins. An arrival that
  changes nothing bumps nothing, which is what ends an exchange.
- **`plan_carriers`** holds one row per repository and carrier (`git`, `hub`):
  the peer, how far into `plan_changes` the carrier has taken (`sent_seq`), the
  hub's watermark read to (`seen_seq`), the last error, and how many records
  its last read brought that this Machine could not write, with why
  (`not_taken`, `not_taken_reason`). A carrier's
  unsent records are those past its `sent_seq`. Sending one twice changes nothing, so
  there is no per-field queue and no sent-but-unconfirmed phase.

Comments, ownership entries and created records only append. Deletion is a
field. Arriving comments carry ownership entries into the Task's log and steer
the Task with a person's direction comment, as a local comment would, whichever
carrier brought them. A claim is first-wins: an ownership entry names the
entry in effect when it was written and carries its order from its writer's
plan clock, and a claim whose writer had not seen the claim holding the Task
loses to it ([ownership](machines.md)). The losing Machine stops its run and
keeps the work. `lf task claim` and `lf task run` on an
unowned Task exchange first, briefly, and go ahead if that fails.

**Git.** `refs/loopflow/plan/<machine id>` on the repository's remote holds that
Machine's whole plan, whoever wrote each field: one canonical-JSON file per
record (`tasks/<2 hex>/<id>.json`, `waves/<id>.json`, `projects/<id>.json`,
`comments/<2 hex>/<id>.json`) and `repository`, the repository id. Only the
owning Machine pushes its ref, so a push is a fast-forward and is never forced.
One exchange fetches `refs/loopflow/plan/*` into `refs/loopflow/plan-seen/*`,
reads the files that changed in each other Machine's ref since the tip in
`plan_refs`, applies them through the one way in, then commits the records
past `sent_seq` onto its own ref and pushes. It uses no index and no worktree,
and runs outside any SQLite transaction. A file that does not parse, or a
field this build does not know, is skipped and logged; the rest applies. A
record, from either carrier, that cannot be written here, such as a Wave
whose name another active Wave of the repository already holds, is not
taken, with its Projects and Tasks, and the rest applies. The import returns
those records. Git then leaves that ref's tip out of `plan_refs`, the hub
carrier keeps its `seen_seq`, and a hub answers `plan.apply` with their count
so the sender keeps its `sent_seq`: each carrier brings them again at every
exchange until they can be written. `plan_carriers.not_taken` holds their
count and `not_taken_reason` why the first was not (`wave design from mini
…`), set and cleared by a read; the exchange fails with both
(`3 not taken: wave design from mini …`), so `lf task sync` exits 1, and the
carrier's own `error` stays empty. Each
clone keeps its own repository id and records the id every fetched ref names
as a locator of its checkout (`locate_repository`), so a hub or a move that
names the repository by another Machine's id finds it and no saved Task
changes repository. A repository with no remote writes no ref. A gone Machine's ref can be deleted once any Machine has fetched it.

**Hub.** A Machine whose `hub:` setting names another sends the records past
`sent_seq` in `plan.apply` and reads `plan.changes { repository, since }` page
by page; order does not matter, and the first exchange sends the whole plan
once. The hub applies the same rule as everyone and reaches the repository
through its own clone. The carrier is `ssh <hub> lf server connect`
(`ops/plan_hub.rs`). Clearing the hub deletes its carrier rows and loses
nothing.

**When they run.** `ops/plan_carriers.rs` has two verbs. `send` follows every
save, from the CLI or the server: the hub takes the records past its cursor and
Git pushes this Machine's ref as last fetched, with no fetch, both within five
seconds. While Git holds something not taken the save fetches and reads as
well, since the save may be the rename that lets it arrive. A push that fails, runs out of time or is refused (the remote's copy
of the ref is ahead of the one fetched) leaves its records unsent. `sync` reads
and sends by each carrier: `lf task sync`, `lf wave status --sync`, before a
claim, before `lf wave create` or `lf task create --wave` makes a Wave (five
seconds at most, so a Wave of that name made elsewhere is joined, not made
twice), on a Task lookup that misses, and after each command the server
answers, one exchange per repository at a time and at most one every five
seconds after the last. One server task runs the Git exchange for
every repository that has a remote and no tracker every 60 s, which is how
another Machine's change arrives with no hub; with no server it waits for
`lf task sync`. The first `lf task list` or `lf repo tracker` in a clone the Machine
has saved nothing in does one exchange before it reads, which registers the
clone for the server's minute. A served read of such a clone answers with an
empty plan once and exchanges after its reply. Task creation reads
nothing first: the send after its save takes in what changed at the hub. A
provider exit reads the hub only; `lf repo tracker --subscribe` reads the
store alone. Every
exchange a command waits on without asking for it (a save, a claim, a first
read, a read of the hub) is stopped at five seconds: Git is killed and the
hub's answer is waited for until then, and the next exchange carries the
rest. `lf task sync` waits as long as the carriers take.

A change costs one small commit per Machine on the remote; sized for about ten
Machines per repository. Tracker repositories use none of this:
`tracker_actions` below, per Machine.

### One outbound queue

Every change Linear or Asana carries is one `tracker_actions` row, written in the
transaction that saves it: `create`, `field`, `order`, `state`, `comment`,
`delete` or `link` (a Project's Initiative, a follow-up relation
`relation:<id>`, a pull request `pr:<url>`). Each row addresses one owner, a
Wave, Project or Task, on the tracker named by `tracker`, set when it is queued
from the owner's binding or queued creation. `value_json` is a `WorkValue` in
store words; `linear/actions.rs` and `asana/actions.rs` turn it into a request
and nothing else does. `WriteOrigin::Local` writes queue; applying tracker events or another Machine's records queues nothing, so incoming edits never echo. A newer queued value for the same owner and subject
retires the older one; a sent one stays for its look. An owner with no
binding and no queued `create` queues nothing.

One bounded pass (`ops/tracker/drain.rs`) sends at most 20 requests or runs 5 s,
four owners at a time, each request under its own 5 s deadline. Owners go in
order of their oldest action, except that the owner a command just saved goes
first. Within an owner, everything waits behind an unconfirmed `create`; otherwise
only actions on the same subject wait behind each other. A Task naming a Project
still being created, and a relation to a Task with no tracker object, hold
without a request. So does an action its tracker blocks (a Linear Project
whose Wave has no Initiative): the block is decided before the send, saves its
error once and keeps the action queued at no cost to the pass. A pass
with nothing unconfirmed asks the tracker nothing.
`lf task sync <id>` drains one owner now, looking for every sent action at
once; without a tracker it exchanges the repository's plan by Git and with the hub.

`planning::rules::decide` confirms each kind against what the tracker holds,
for the drain and for inbound events alike, or marks it conflicted: a field
adopts a newer tracker value and keeps the losing one in `conflict_json`; state uses Linear's history
when it has one; order moves only on complete-list evidence; deletion is confirmed by
trash or a delete acknowledgement and retires when the object is active at a
newer revision; a comment matches its id, or on Asana its visible text.

The lost-reply rule is the same for every kind: a `sent` action is looked
for before anything else on its subject is sent, at most once per
max(600 s, the delta freshness window). A send that carries our id or reads
before it adds (Linear comments, every link) is repeated instead. A tracker with
no form for an action answers `Unsupported` and the action retires with that
reason: Asana has no Project edits after creation, no order, no Initiatives and
no follow-up relations. A queued action for a tracker the repository no longer
uses retires naming both.

A `create` carries the owner's UUID, which both trackers send as the tracker
identity, and its creation baseline: shared creation fields, never a
tracker-shaped copy or local execution data. The tracker builds the request when
it sends. The event that finds it attaches the tracker mapping to the original local row
and confirms the `create`. An unknown creation outcome stays sent until a
look finds it; a matching title proves nothing.
Comments retain identity and authorship. Incoming planning completion changes only
the planning disposition: it neither moves a local Task nor authorizes cleanup.
Session/Process records and checkout paths never travel to another Machine.

One view answers "where does my change stand" on every tracker:
`TrackerActionStatus` (`planning_sync.rs`). An action is open while `queued`
or `sent`, and lost while `conflicted`, with the losing `local_value` and the
winning `tracker_value`. `lf repo tracker` lists the repository's open and lost
actions ("3 open · 1 lost"), and a Task's or Project's `sync` is its slice. A
lost action retires when it is answered: a later save of its subject
("answered by a later save"), an event that changes that field again at a
newer revision ("changed again in Linear"), or `answer_lost` over action ids
of any owner (`lf repo tracker answer <id> --keep-theirs | --use-mine`, served
as `repo.tracker.answer` for Desktop's Keep / Use mine on every line; `lf task sync
--keep-theirs | --use-mine` answers a Task's whole slice). Without a tracker,
`lf repo tracker` has one line per carrier, `Plan: Git origin, 2 unsent.` and
`Hub mini.`, each with its unsent records by identifier, its last error and
what it brought that was not taken (`Plan: Git origin, 3 not taken.`), or
`Plan: this Machine.` with no remote and no hub; `--json` carries the same as
`plan: {git, hub}`, each `{remote | machine, unsent, records, error,
failing_since, not_taken: {count, reason} | null}` or null; `unsent` counts and `records` names the first twenty. Linear checks run
at the boundaries described below. A short save attempts delivery after its
answer. No resident, whole-store callback, scheduler or
implicit turn/Graph replay is involved.

`--machine` sends portable pushed-code requirements and a Task selector, then the
target resolves its own configured planning. Linear mappings resolve independently
retained legacy IDs. Machines without a tracker share every planning ID. A source Task awaiting
Linear creation is unavailable remotely until publication succeeds. Fetch precedes
code decisions, including branches without PRs. Repeated dispatch reuses the existing
Task and checkout; unpushed source work and a target missing the requested commit
produce actionable errors without committing, resetting or replacing local work.

The personal namespace and tracker-first planning writers are deleted. Connected
CLI and Desktop project creation, field/order edits, state and comments through
`PlanningSyncStatus`, derived from their existing actions. Unmapped creation is
pending in connected repositories; attempted effects retain uncertainty and errors.
Observed conflicts show both values after adopting Linear. Disconnected repositories
show no pending Linear delivery; retained losing values remain inspectable.
Planning refresh runs at useful boundaries: before Graph steps, after agent
turns, before Linear mutations, and when CLI commands show planning. One team
query obtains issues and comments changed since its successful watermark; large
deltas paginate without advancing that watermark early. SQLite shares a ten-second
attempt stamp per team across repositories and processes; a refresh lock
covers the short request/ingestion operation, with no resident owner or hand-off.
Failed checks keep the prior watermark and local planning.

Teams without live Tasks are not checked. Locally finished/abandoned identities
are excluded from refresh. Explicit detail reads remain available. Desktop and
`--subscribe` alone own a clock: 30–60 seconds initially, backing off to four minutes
while unchanged. Closing the watcher stops it; idle agents own no Linear timer. Hub exchange has no clock of its own; the Git fetch is the server's one 60 s timer.
Projection functions remain local; command boundaries acquire before projecting.

Linear HTTP 400 rate-limit errors, 429 and exhausted request/endpoint/complexity
headers establish a Home-wide pause across repositories and refreshed logins.
Reset headers are epoch milliseconds; missing deadlines fall back to one hour.
In-flight requests may finish, and a successful final response remains usable.
One warning announces the window; later calls return its deadline without network
access. After reset, the next boundary retries pending identities. Successful
mutation payloads supply confirmation without a follow-up read. Lost responses
retain uncertainty; positive rate-limit rejection makes that action retryable.
Independent Homes do not share this pause.
Ingestion adopts observed Linear conflicts and keeps the losing local value on
its action. Retired actions never reenter delivery after a late acknowledgement
or matching event.
The documented unconditional-update race remains a protocol limit.

## Plan creation and selection

```bash
lf wave plan bind product <project-uuid> --json
lf wave create product --json
```

Validate one existing Project and select its accepted local record on the Wave's
SQLite row (`waves.current_project_id`). Repeating the same binding keeps it.
Use `--replace` to switch an existing selection. Setup preserves tracker status,
content and identity, including a Backlog Project. It does not
activate the Project. Operation routing and status select this exact ID; the JSON
Project summary carries a required `current` boolean for Desktop. Status and
`plan list` retain predecessor plans and Tasks. Registration and unstarted managed
work require that exact Project to be In Progress; another In Progress Project
does not compete with the binding. Already-started Tasks retain continuation in
predecessor Projects.

Wave and `plan list` Task summaries read saved Work directly: `id` is the local Task
identity, `tracker_id` is its optional tracker binding, and `sync` is required
read-only evidence. Missing tracker inventory does not erase saved prose, due
dates or assignments. Graph, execution and checkout evidence remain separate.

`ensure` activates the configured Backlog or Planned Project in SQLite and records
its status change for delivery. Without a binding, creation, original name,
selection and the local transition action commit together. Retry reuses that
identity and preserves later edits. Existing tracker creation reservations retain
their UUID and mappings. No tracker request is needed, including during an outage.
Terminal, archived, paused or foreign Projects retain their history. Names and content remain intact; the Wave owns its default graph. Binding accepts the durable or mapped ID of
an already saved Project; it does not fetch a missing Project. Legacy YAML import
retains the original bytes and requires that exact Project's saved record.
Project export uses the original transition action on the foreground connection.
Creation and Initiative attachment retain separate attempted effects and exact
mutation acknowledgements; a missing response never permits another create or attachment.
Mapped status actions use foreground delivery;
a locally active Project alone is not proof of Linear activation. Ensure never searches for candidates; status and `plan list` never call it. Desktop
ensures on explicit opening or retry while retaining cached planning and independent
Session reads. Both primary and Portfolio surfaces render the same SQLite-derived
`project_readiness` through `lf wave show --subscribe --json`. Committed selection,
accepted facts, transitions and exact activation Process outcomes invalidate that reading.
A pending transition or unfinished Process is unresolved evidence, not proof of liveness.
Watchers and status reads never provision or import.

The first explicit ensure or binding setup imports an existing
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

## Replace a Wave's Project, keep the work

```bash
lf wave edit-project product --name "2026-10 — customer work"
lf wave update-project --wave product --file scratch/product-project.json
lf wave plan bind product <Project UUID> --replace
```

`update-project` replaces the current Project's KRs and metric targets with the
supplied JSON (`{"metric_targets":[],"krs":[]}`); names, summaries and
unrelated content stay. `bind` moves the Wave's shared selection to an
existing non-terminal Project. Neither moves a Task: started Tasks keep their
identity, checkout, PR and captured execution where they are, and `lf task
move` is the one way a Task changes Wave. There is no rotation operation.

## Inspect planning before starting execution

```bash
lf task status INF-124 --json      # the Task row, Sessions, Graph processes, comments
lf task show INF-124 --json     # the planning evidence below
```

`lf task show` returns canonical `task` and `project` records, `tracker_state`
(with `state` and `synced_at`), `plan_error`, `plan_stale`, delivery `sync` and optional
`execution`. Local IDs and explicit Linear bindings stay distinct. A foreign or
Project-less event returns refresh evidence without fabricating owned
Work; its concrete record remains retained internally. Status may acquire planning
but never completes a Task, allocates a checkout or moves a Task's graph.
Wave, `plan list` and Work frames expose the same refresh state and age separately
from saved content and outbound delivery.

Exact lookups and Wave views share normalized SQLite entities scoped to a
repository and tracker. An empty cache fetches the requested issue before
reporting absence; tracker errors remain resolution failures. Bounded refresh
retains the last successful event and its timestamp on failure. Lists
reference shared Project/Task facts; omission alone never proves deletion.
A null detail response invalidates cached admission until a complete detail
read repairs it. Confirmed deletion actions continue to exclude removed Tasks,
including after stale list ingestion.

Status preserves dated facts even after hard-stale or forced refresh fails.
`tracker_state.state` distinguishes `available`, `unavailable` refresh, `invalid`
evidence, confirmed `removed` planning, and an `absent` detail response. A null
tracker response observes absence for that refresh; the stored record stays
invalid until complete detail repairs it. This creates no deletion action.
`tracker_state.synced_at` records the last successful sync. An unresolved
selector returns an unavailable envelope even without execution.

Invalid and removed facts remain inspectable and withdraw new-launch availability,
without settling or stopping existing execution. Saved Work supports offline
admission; time since the last refresh and missing inventory alone do not erase it. Wave
`synced_at` dates its last successful list; entities may have newer detail evidence.

Task facts carry Linear's `updatedAt` as `revision`. Detail, list and confirmed
mutation refreshes share one writer: older tracker revisions cannot overwrite
newer facts, and conflicting facts at an equal revision fail without replacement.
Complete responses must include nullable fields; omission cannot clear known data.
Stored change events invalidate planning even without execution. A complete read
at or beyond the event's revision repairs the invalidation. Removal events fence
later reads, including when the event arrived before the issue was cached.
Events do not replace complete planning entities. The former daemon webhook
ingress is removed; these store operations do not establish live event delivery. Project facts also
carry `revision`; a newer Project event updates independently of the issue's
revision. Older events cannot overwrite it. Project responses must include
nullable content fields and relationship sets.

Private Linear inventory/detail refresh commits retained events and
Projects/Tasks in the same SQLite transaction. Public Store APIs expose canonical
Work and narrow refresh evidence, never fetched response models. Ordering
actions receive accepted local Task IDs; new disposition conflict actions retain
revision/state/completion evidence. Historical actions keep their original bytes. Projection uses each stored entity's
refresh time, never the Wave's aggregate sync time. A projection failure rolls
back the event and its Work changes. It preserves execution fields and retained identity;
an unknown destination does not authorize a Task transfer. Restart changes
execution without rewriting accepted planning. A single Project event does
not advance the Wave's full-refresh timestamp. Reteam accepts complete issue
readbacks through that same owner and reconciles confirmed Team relationships
without changing Initiative ownership or independently newer Project facts.

Full and partial Wave ingestion validate the accepted Project's Initiative before
recording membership or freshness. Detail refresh resolves that Initiative through
Wave configuration and the registry, then accepts the association and projects
facts in one transaction. A durable Project row alone grants no Wave ownership.
Unconfirmed cold detail retains durable plans; foreign or unmapped ownership at
the operation boundary reports an error. Both Project and Task projection require
an exact accepted Initiative match. Snapshot refresh and cold-detail readback
hold the Wave planning lock through the SQLite commit. Queued workers retain
shared ownership after their async caller is canceled. Reteam acquires
participating Wave locks in stable ID order; already-held refresh paths reuse them.
Cold detail discovers ownership, locks, then reads ownership again. Null detail
invalidates only the cached revision and event it queried; it cannot invalidate
a newer accepted Task. Readback follows the issue UUID across identifier changes.

The `project_readiness` migration preserves original Linear Project bodies and
refresh evidence before a one-time name/slug correction from fresh tracker
facts. Only pre-cutover rows receive this exception; all non-name conflict checks
remain, and later equal-revision name conflicts are rejected. Rejected ingestion
cannot consume the exception or update durable Project facts first.

List coverage never removes a Project merely because a later response omits it.
Without removal evidence, refresh fails and retains the previous event.
Project revisions do not establish ordering for separate Initiative/Team
relationships. A contradictory relationship set stays unresolved, retains its
last-good facts, and blocks managed readers. Replaying a list or detail does not
clear that uncertainty. Explicit reteam reconciles the exact confirmed Team set
under refresh ownership; it cannot reconcile a changed Initiative.

Historical tracker transitions and archival acknowledgements remain evidence of
the removed rotation; nothing infers that their uncertain effects settled.

## Capture a Graph once

```yaml
# .lf/graphs/build.yaml
- step: implement
- step: compress
- step: gate
```

```bash
lf graph build
lf graph show GRAPH_RUN --json
```

A Graph is one lf process and the step processes it starts; its ID is the Graph
process's. Starting one compiles its definition, including every router and
alternative, into a graph the Graph process holds in memory. One cursor and its return
counters identify loop passes. Subgraphs and passes are display lenses over the
step processes, with no separate record or process.

Every Graph started for a Task, by naming it or in its checkout, is equally that
Task's work; none is selected or privileged. Taskless and Task execution share the
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
the run's next attempt sets out from, and `lf_process_id`, that Task run. Whether
it still runs is read from the Process, never stored. `lf task run` writes
`run_start`; the child Graph process overwrites it after every step with the
cursor of the next one (loop counts, iterate direction, pending verdict and route
included), so a failed attempt and the next `lf task run` start at the step that
failed and the steps before it never rerun. The child Graph process writes where
it left the Task: before the interactive step it stopped at, or `end`. With no
run, `lf task run` continues after the step the Task waits at; `--from STEP` (a
step id, skill or composed graph name) starts anywhere, and starting from an
interactive step only puts the Task there. `-b task run` runs
straight through headless, and `lf task auto on` turns auto on and runs it. Other Graphs in
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
with nothing running, `moving` while a run carries it, whatever its auto
setting; abandonment is its own mark.
An unfinished Graph or step in the Task's work also makes it `moving`,
including a Graph started before checkout adoption.
That execution does not add the Graph to the Task's history.

On-demand team checks acquire issue membership/state independently of
comment and state delivery. Tracker actions retain their original tracker revision,
attempt evidence and conflicting value. The latest action determines the displayed
writeback state; Tasks store no separate copy. A response records only its captured
effect, so it cannot settle a newer decision. State refresh and delivery share
one transactional reconciliation rule: observed conflicts adopt Linear and settle
the losing action without moving the Task. No local/tracker clock comparison orders edits.
The tracker read and mutation remain separate requests; they do not prevent a
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
This is an explicit concurrency limit, not a tracker atomicity guarantee.

Abandonment saves the decision and cancellation
action together without tracker access. Cleanup failures preserve the decision;
unknown or live execution prevents cleanup before tracker inspection. Cancellation
uses the same state delivery path as completion and reopening. Resolve the issue
team and target state before marking an action attempted, so failed reads remain
retryable. A lost mutation reply retains uncertainty until the next tracker event.
Delivery at save, check and watcher boundaries selects unmapped local Projects and Tasks,
using their saved UUIDs. Each `create` action retains its baseline, attempt and
error; no second creation owner exists.
Exact events attach mappings in the common ingestion transaction before
inventory can allocate another local identity. The creation snapshot establishes
field baselines without acknowledging later edits. Observed Linear conflicts still
win and retain the losing action. Removed Tasks export only when an attempted
creation needs reconciliation, then their existing deletion action owns removal.
Creation action changes advance the planning revision, so active readers acquire
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
answer is corrected by `lf -b steer ID TEXT` in the same
conversation, at most twice, then the Graph fails; provider failure remains
distinct.

Blocked records the reason and stops at the current Graph position. Existing logs
and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. Nothing retries
or resumes a stopped Graph.

## Inspect before further work

```bash
lf task status INF-124
lf graph show GRAPH_RUN --json
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

`task run` takes up the Task's captured graph and starts a fresh Graph process
from its saved position. It returns after registration; `-b` waits and runs
interactive steps headless. Ordinary runs stop at the next interactive step for
the Task conversation. A direct `lf --task ISSUE graph GRAPH` uses the same runner
but does not move the Task's position. `--reason` publishes direction first.
Inspect live and unfinished processes before launching more work.

Task status and `plan list` separate the Task's graph position from `execution`,
which describes its latest Graph as `idle`, `starting`, `running`, `stalled`,
`blocked` or `unknown`. `run_control` describes the available action; recorded
history alone is not permission to start a competing Graph. See
[interactive steps and Task states](../orchestration.md#where-a-person-takes-part).

Automatic provider retries within one Process retain earlier failure and usage and
select only an authorized successor. An uncertain mechanical effect requires
inspection; moving a cursor does not establish exactly-once external effects.
Missing process evidence is uncertainty, and causal ancestry grants no signal
authority.

## Task conversations

```bash
lf session list --json
lf resume SESSION --prepare --json
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
Task selects its primary conversation without reopening the Task; earlier usage
attribution remains unchanged.

```bash
lf steer INF-124 "keep the public name"
lf --wave product wave-operate "recover the stopped release Task"
```

SQLite owns every Task comment. A save records the comment and its stable delivery
UUID in one transaction, before contacting Linear. CLI and Desktop read the thread,
pending IDs and losing local bodies from one SQLite snapshot. An observed comment
conflict adopts Linear’s body, author and time while the action retains both
complete comments; no replacement comment is created. Connected repositories show
pending sync, including comments on unmapped Tasks. Saves and on-demand boundaries
attempt delivery; team checks fetch incoming comments independently. Desktop and
`--subscribe` continue checks while open. Lost replies are resolved by exact UUID,
issue and body. Incoming comments enter the same thread without echoing locally
saved steers. Comments and adopted definition revisions save immutable
steers. Each Session selects steers from checkout/explicit Task associations, Wave
ancestors and repository. Provider callbacks deliver bounded batches at startup,
continuation and tool boundaries; the Task brief reloads independently of consumed
steers. Idle steering starts no Graph. Local emission
remains unconfirmed and does not prove the model followed a correction. Wave
planning uses ordinary finite LfSessions.

Work reservation claims an unowned Task in its ownership log; an inspection Process does not. Constructors
validate Task/Wave ancestry, and Wave moves preserve historical event
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
