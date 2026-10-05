# Planning

```bash
lf --wave product wave/operate
lf task status INF-124 --json
lf checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf --task INF-124 flow start
lf repo new-chapter 2026-10 --dry-run
```

Wave → Task is the navigation hierarchy. A Wave keeps its objective, memory,
cadence, budget and metric instruments across plans. Its one In Progress Linear
Project owns Tasks, KRs, targets and the default Flow. A Chapter is the shared
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
for adoption, default Flow and disposition details.

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

Invalid and removed facts remain inspectable through status. Managed execution
requires available planning: a fresh cached observation suffices, but a failed
due refresh stops continuation. Wave `synced_at` dates its last successful list
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
lf flow show FLOW_SESSION --sessions --json
```

Starting a Flow compiles its definition into one FlowSession's captured graph,
including every Skill, router, alternative and review policy. Source edits or
deletion cannot change it. One cursor and its return counters identify loop
passes; retry retains the same pass. Subflows and passes are display lenses,
with no separate FlowSession, process or claim.

A Task selects one managed FlowSession and may have other attributed Flows.
Taskless and managed execution share the driver. The Project's Flow supplies the
default for a new selection; explicit selection is allowed. Continuing a saved
Flow preserves its definition, review wait, failure and feedback. Explicit restart
replaces it. Finishing retains history, clears the managed selection and chooses
no successor; Flow completion alone does not complete Task Work.

## Settle the exact boundary

A boundary is the FlowSession, node and iteration tuple. Its claim and version
fence each mutation. Agent boundaries select a native start in an AgentSession
and consume its exact successful completion once. Mechanical boundaries retain
correlated starts and results in Flow history. Each executed skill or operation
runs in its own child lf Exec through the ordinary command path. The Flow driver
owns navigation; a child's command outcome alone cannot settle agent work.

A deciding step returns a JSON `decision`: `advance` or `iterate` with a nonempty
`summary`, or `blocked` with a nonempty `reason`. The unused field is null; all three
keys are required in the provider schema. Settlement also accepts persisted receipts
that omitted the unused field. A router returns a JSON object containing
`path`, constrained to the captured branch's path names. Each provider receives
the schema before generation. Session history retains the native output and
completion separately; only the exact selected successful result is consumed
inside the Flow's fenced settlement transaction. Invalid output receives at most
two corrective turns in the same conversation; provider failure remains distinct.
Helpers, older successes and late generations cannot settle the current selection.

Blocked records the reason and stops at the current Flow position. Existing logs
and outcomes provide the evidence. The Wave operator resolves
impediments or discusses missing judgment in its ongoing chat. Explicit retry
retains the position and pass; unchanged failures do not automatically retry.

## Inspect before further work

Saved Flow invocations are history, not a public resume entry point. Inspect the
selected Session, process and effect receipts before launching a fresh ordinary
Flow. Observation does not replay work or consume a surviving child's result.

The remaining managed Task controls below are part of the pending worker removal.

```bash
lf --task INF-124 flow start
lf --task INF-124 flow start --retry
```

The Flow orchestration claim and conversation driver claim have different owners.
After driver loss, read back the selected native turn from a surviving engine;
do not send extra input. Its completion retains original Exec and provider
generation, even when a new driver records it. The old command outcome stays
unknown if no terminal receipt exists.

Explicit retry after confirmed engine exit resumes the conversation with a new
provider generation. Exact process/native evidence excludes an old writer.
Automatic provider retries within one Exec retain earlier failure and usage and
select only an authorized successor. An uncertain mechanical effect requires
inspection or explicit retry; moving a cursor does not establish exactly-once
external effects.

Task commands supply the same explicit attribution and shared Flow execution;
their claim and background placement are driver mechanics.
A live claimed worker cannot be replaced because a status read timed out. Missing
process evidence is uncertainty, and causal ancestry grants no signal authority.

## Task conversations

```bash
lf session list --json
lf session connect SESSION --json
```

Discuss design and review feedback in the ongoing Task conversation. Historical
review feedback remains Session evidence. No Ready/Complete operation closes the
conversation or releases a saved Flow. The caller inspects outcomes and effects
before selecting further work; pane closure and provider exit grant no authority.

Saved handoffs retain executable, Home and database together. Renaming, binding
and driver replacement retain conversation identity and feedback. Desktop reads
the same record and keys its terminal surface on AgentSession identity.

## Work, steering and execution

Task status is `Ready`, `Done` or `Abandoned`. Current activity, command outcome,
conversation state and Flow progress remain separate. A ready Task can have no
live process; an absent terminal result cannot prove liveness. Binding to a done
Task assigns work history without reopening it or acquiring its managed claim.

```bash
lf comment INF-124 "keep the public name"
lf --wave product wave/operate "review the current priorities"
```

Linear owns authored Task comments. The shared skill path supplies Task context
and live steers when the checkout or explicit attribution selects a Task.
Idle steering starts no worker. Prompt inclusion and provider acceptance do not
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
These captured plans serve both Task and ordinary execution; resume never
resolves names from current source.

Review Sessions launch the captured Skill explicitly, escaping command names.
Execution selection reads that captured value before looking for authored files;
help and list continue to inspect local definitions without Session state.
