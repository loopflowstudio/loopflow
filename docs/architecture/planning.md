# Planning

```bash
lf --wave product wave/operate
lf task status INF-124 --json
lf task checkout INF-124
lf --task INF-124 research "write scratch/runtime.md"
lf task run INF-124
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
lf wave sync --wave product
```

A Planned Project expresses the next plan. Rotation reuses the explicitly named
successor or creates one with the predecessor's Flow. Started unfinished Tasks
move with identity, checkout, PR and captured execution intact. Proven untouched
backlog is canceled; completed Tasks stay historical. Missing local or provider
evidence cannot authorize retirement. Linear keeps the Projects and their Tasks.

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

Invalid and removed facts remain inspectable through status. Managed readers
exclude them, and retain their existing hard/forced-refresh refusal. Inspection
does not select an outage-admission policy. Wave `synced_at` dates its last
successful list acquisition; joined entities may have newer detail observations.

Task facts carry Linear's `updatedAt` as `revision`. Detail, list and confirmed
mutation refreshes share one writer: older provider revisions cannot overwrite
newer facts, and conflicting facts at an equal revision fail without replacement.
Complete responses must include nullable fields; omission cannot clear known data.
Signed issue webhooks invalidate planning even without execution. A complete read
at or beyond the event's revision repairs the invalidation. Removal receipts fence
later reads, including when the event arrived before the issue was cached.
Webhook payloads do not replace complete planning entities. Project facts also
carry `revision`; a newer Project observation updates independently of the issue's
revision. Older observations cannot overwrite it. Project responses must include
nullable content fields and relationship sets.

List coverage never removes a Project merely because a later response omits it.
Without removal evidence, refresh fails and retains the previous observation.
Project revisions do not establish ordering for separate Initiative/Team
relationships. A contradictory relationship set stays unresolved, retains its
last-good facts, and blocks managed readers. Replaying a list or detail does not
clear that uncertainty; acquiring ordered relationship evidence remains future work.

Chapter rollover records the provider's successful archive acknowledgement before
refreshing the current plan. Archived predecessors leave current views, retain
history, and cannot return through a delayed list. Completed chapter receipts
preserve this evidence on migration. This does not implement provider Project
completion or restoration semantics.

## Capture a Flow once

```yaml
# .lf/flows/build.yaml
- skill: implement
- skill: compress
- skill: gate
```

```bash
lf flow build
lf flow resume FLOW_SESSION
```

Flow is the reusable definition; FlowSession is one captured execution. Capture
expands template composition and saves every Skill, router, alternative and
review policy. Source edits or deletion cannot change a saved execution. The
record owns graph, cursor, return counts, nullable Task/Wave, current boundary,
claim and completion. Only actual runtime loop nesting creates child FlowSessions;
template composition does not. Parent and child nullable Task ancestry agree.

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
correlated starts and results in Flow history; they create neither fake agent
conversations nor synthetic Execs. One actual lf process can execute several
boundaries and then fail; its command outcome is not every boundary's outcome.

A deciding step declares a JSON object containing `decision` (`advance` or
`iterate`) and a nonempty `summary`. A router declares a JSON object containing
`path`, constrained to the captured branch's path names. Each provider receives
the schema before generation. Session history retains the native output and
completion separately; only the exact selected successful result is consumed
inside the Flow's fenced settlement transaction. Invalid output receives at most
two corrective turns in the same conversation; provider failure remains distinct.
Helpers, older successes and late generations cannot settle the current selection.

A blocked decision opens one keyed Ask. Completion returns saved feedback for
reassessment at the same boundary, never a navigation verdict. Retry preserves
the answer instead of opening duplicate conversations.

## Recover without inventing an outcome

```bash
lf task run INF-124
lf flow resume FLOW_SESSION
lf flow resume FLOW_SESSION --retry
lf task run INF-124 --retry
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

Managed dispatch keeps Task agent choice, adoption checks and unblock policy.
A live claimed worker cannot be replaced because a status read timed out. Missing
process evidence is uncertainty, and causal ancestry grants no signal authority.

## Ask and review

```bash
lf ask "Review which migration should survive"
lf session list --json
lf session connect SESSION --json
lf session ready "Feedback and remaining work"
lf session complete SESSION
```

Ask opens a durable AgentSession in the caller's checkout and waits for explicit
completion. A keyed retry returns the stored answer without a provider launch.
A Flow review opens its captured Skill and retains exact Flow membership. Ready
saves feedback; Complete persists it before provider teardown and successor
launch. The following step receives the feedback; a later decision chooses
navigation. Pane close, provider exit and readiness do not complete a review.

Saved handoffs retain executable, Home and database together. Renaming, binding
and driver replacement retain conversation identity and feedback. Desktop reads
the same record and keys its terminal surface on AgentSession identity.

## Work, steering and execution

Task status is `Ready`, `Done` or `Abandoned`. Current activity, command outcome,
conversation state and Flow progress remain separate. A ready Task can have no
live process; an absent terminal result cannot prove liveness. Binding to a done
Task assigns work history without reopening it or acquiring its managed claim.

```bash
lf task comment INF-124 "keep the public name"
lf --wave product wave/operate "review the current priorities"
```

Linear owns authored Task comments. Only the claimed Task advancer attempts live
delivery; independent attributed conversations receive their own launch context.
Idle steering starts no worker. Publication, prompt inclusion and provider
acceptance are distinct evidence and do not prove the model followed a correction.
Wave planning is an ordinary finite AgentSession; no resident operator is needed.

Work reservation sets Task Started once; an inspection Exec does not. Constructors
validate Task/Wave ancestry, and chapter transfers preserve historical event
attribution. Task delivery separately owns Git/PR mutations. Conversation identity
and Flow membership never substitute for that authority.

[Delivery](delivery.md) follows Task Work through Git and GitHub.
[Execution](execution.md) owns process and conversation admission.
