# Planning

```bash
lf --wave product wave/operate
lf task status INF-124 --json
lf checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
lf repo new-chapter 2026-10 --dry-run
```

Wave → Task is the navigation hierarchy. A Wave keeps its objective, memory,
cadence, budget and metric instruments across plans. Its one In Progress Linear
Project owns Tasks, KRs, targets and the workflow. A Chapter is the shared
name of those current Projects across the repository. This page specifies the
accepted model; [cutover status](../architecture-reference.md#cutover-status)
records the remaining implementation and proof gaps.

## Rotate the plan, preserve the work

```bash
lf repo new-chapter 2026-10
lf refresh product
```

A Planned Project expresses the next plan. Rotation reuses the explicitly named
successor or creates one with the predecessor's Flow. Started unfinished Tasks
move with identity, checkout, PR and captured execution intact. Proven untouched
backlog is canceled; completed Tasks stay historical. Missing local or provider
evidence cannot establish that work should be retired. Linear keeps the Projects
and their Tasks.

Rotation reads fresh provider state after each interruption. A partial transition
to the requested name is recoverable using stable Project identities and one
unambiguous predecessor group. Unrelated competing current plans remain unresolved;
newest-looking names never win. Status mutations do not form a distributed
transaction. Another Home observes the new plan through normal synchronization.
There is no Chapter row, packet or local switch. See [Waves](../waves.md#the-planning-model)
for adoption, workflow and disposition details.

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

List coverage never removes a Project merely because a later response omits it.
Without removal evidence, refresh fails and retains the previous observation.
Project revisions do not establish ordering for separate Initiative/Team
relationships. A contradictory relationship set stays unresolved, retains its
last-good facts, and blocks managed readers. Replaying a list or detail does not
clear that uncertainty; acquiring ordered relationship evidence remains future work.

Chapter rollover transfers unfinished work and settles backlog before completing
each predecessor Project in Linear. It confirms provider completion before recording
the Project. Closing the Project does not complete transferred Tasks or change
historical KR results. Current Project selection follows provider status.

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
lf flow show DRIVER_EXEC --sessions --json
```

A Flow is one driver Exec and the step Execs it starts; its ID is the driver
Exec's. Starting one compiles its definition, including every router and
alternative, into a graph the driver holds in memory. One cursor and its return
counters identify loop passes. Subflows and passes are display lenses over the
step Execs, with no separate record or driver.

Every Flow naming a Task, or run in its checkout, is equally that Task's
work; none is selected or privileged. Taskless and Task execution share the
driver. Each `task run` starts a fresh Flow as a child `lf run`, and a fresh
one again when that Flow exec fails. Flows hold autonomous
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

Position is stored: at a node, or on an edge with the Exec carrying it.
Whether that Exec still runs is read from the Exec, never stored. The store
has one read and four writes, and every write appends to
`task_workflow_moves`:

| Call | Effect |
| --- | --- |
| take up | store the graph at `start` |
| choose | from the edge's `from` node, or from a stopped edge that left it: on the edge, carried by the choosing Exec; straight to `to` for an edge that runs nothing. Refused when the Task has moved since it was read |
| arrive | the Exec that carried the edge, on success, puts the Task at `to` if it is still on that edge |
| set | put the Task at a named node |

`lf task run ISSUE [NAME]` chooses, then runs the edge's Flow in the same
process, which arrives when the Flow succeeds. A Flow that stops leaves the
Task on its edge. `lf task move ISSUE NODE` sets. An edge is not chosen
while another runs. A move's author is its Exec's calling conversation, else
a person; an arrival is the edge's own.

A Task's state is read from that position and stored nowhere else: not ready
with no Workflow, ready at `start`, active at a node or on an edge, done at
`end`. Abandoned is the Task's own mark. Any move that reaches `end` is
completion: one transaction writes the position, the Completed event and the
retirement of an empty unpublished PR, after the settled-PR and
unresolved-execution checks and the Linear write. A Task with no Workflow
reaches `end` on `unplanned`, which has nothing between. Linear calling an
active Task complete is read as `planning_conflict`; `end` then takes `--force`,
kept in the move's note.

## Read each step's result

Each executed skill or operation runs in its own child lf Exec as the plain
command: `lf -b skill <name> [message]`, or the operation's own command. The
step is told nothing about its Flow. The driver owns navigation and the Flow's
record, FlowExec: the Flow's name and graph as compiled at launch, then one
appended row per step with its Exec, graph node key and per-edge iterations.
Nothing reads it back to resume. A step's result is how its process exited; a
deciding or routing step also answers through the final answer of the Session
turn its Exec captured. After an operation the driver stops the Flow when a
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
lf flow show DRIVER_EXEC --sessions --json
lf task interrupt INF-124
lf task run INF-124 --reason "take the smaller approach"
```

A killed driver leaves its Execs and FlowExec rows as history; the last step
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

Automatic provider retries within one Exec retain earlier failure and usage and
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

Saved handoffs retain executable, Home and database together. Renaming, binding
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

Linear owns authored Task comments. The shared skill path supplies Task context
and live steers when the checkout or explicit attribution selects a Task.
Idle steering starts no Flow. Prompt inclusion and provider acceptance do not
prove the model followed a correction. Wave planning uses ordinary finite
AgentSessions.

Work reservation sets Task Started once; an inspection Exec does not. Constructors
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
