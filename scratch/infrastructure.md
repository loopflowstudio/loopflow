# PR #872: a coherent Wave → Project → Task product

## Why this file exists

[PR #872](https://github.com/loopflowstudio/loopflow/pull/872) is an
exploration branch that crossed several product boundaries at once. It began
as a replacement for unsteerable detached task loops and expanded into a new
execution model, a new delivery policy, a Project runtime, a supervision
protocol, registry migrations, CLI changes, daemon deletion, and a passive UI
projection.

The branch has produced useful design evidence. The goal now is to push it into
one coherent end-to-end product, not to partition it into extraction PRs yet.

This file is the branch's single design ledger:

- what changed technically and behaviorally;
- why the change expanded;
- where the current model is internally consistent;
- where it still hides product questions;
- how Waves can direct Projects and Tasks as strongly as possible;
- what Wave Chat must expose for the hierarchy to feel real and trustworthy;
- what remains unanswered before the implementation can converge.

It replaces the branch's prior scratch set. Git retains the detailed history.

## Scope: the published PR and the local continuation

There are two materially different diffs under one branch name.

| Range | Shape | What it primarily contains |
|---|---:|---|
| main → published PR head `bcadf5e7` | 128 non-scratch files, +6,156/−7,668 | Task Sessions, task steering, one-PR-to-main delivery, and removal of the generic loop/queue/stack execution paths |
| published PR head → local head before this synthesis | 39 non-scratch files, +6,327/−667 | Project Sessions, shared child commands, typed observation outbox, decisions, and passive Project projection |
| main → local branch | 135 non-scratch files, +11,851/−7,703 | Both changes combined |

The published PR has 27 commits and no review discussion. The local branch
continued for another ten commits after the published head. Treating all of
that as one review unit obscures the important boundary: Task Sessions caused
the original replacement; Project Sessions were added later to repair a hole
created by that replacement.

## Starting point

Before this branch, Loopflow had two execution systems.

1. A served Wave was a durable, steerable mind. Its listener owned the journal
   and inbox; its resident owned a provider harness. Messages could be injected
   into or queued behind a live turn.
2. `lf loop <flow> <seed>` was a batch worker. It created a worktree, ran
   repeated headless flow passes, watched a loop bit, and reported at
   boundaries. Detached loops lived in tmux but did not expose structured live
   control to the Wave.

Delivery added a third cluster of machinery: stacked runs, queue state,
`combine`, `next`, `lfq`, daemon exec routes, and PR ancestry. A generic
`Run` record carried execution state, worktree placement, lineage, and PR
state. A separate terminal `Session` record described tmux/client attachment.

Two prior changes made that system feel increasingly accidental:

- Wave homes became permanent and stopped rotating on land. Worker worktrees
  became cheap, stable siblings.
- Linear became the source of truth for Wave → Project → Task planning, with a
  SQLite snapshot as the local read model.

At that point the generic worker's free-text identity and stacked delivery no
longer matched the planning model or the desired default of independent PRs to
`main`.

## How the branch became expansive

The expansion followed a comprehensible chain.

### 1. Record-first task execution

The initial product claim was small and strong: concrete work should have a
Linear task before execution, and starting work should be as simple as passing
its identifier to Loopflow.

That required a durable join across systems:

`Linear issue ↔ worktree ↔ provider transcript ↔ process ↔ PR`.

The generic loop's `Run` did not express that join, so the branch introduced
`TaskSession`.

### 2. A task had to survive more than one turn

A PR opening is not the end of work. Review feedback, CI repair, merge
observation, and Linear completion can happen days later. The Task Session
therefore became durable across many provider processes while preserving one
worktree and provider thread.

This changed completion from “the loop bit flipped” to “the PR merged or the
session was explicitly abandoned.”

### 3. Wave-to-task steering exposed a control-plane gap

Codex, Claude, and OpenCode all provide some form of stable child address,
follow-up, redirect, stop, resume, status, and completion notification. The
batch loop had none of that as a reliable parent contract.

Benchmarking those systems expanded Task Sessions from durable placement into
a structured control plane:

- `follow-up`: preserve the current turn; deliver next;
- `steer`: redirect now, using live injection or interrupt-and-resume;
- `interrupt`: stop, optionally replacing pending input;
- durable command receipts and effects;
- wait/resume/attach/abandon;
- decision requests and responses;
- generation-based recovery and atomic turn-boundary settlement.

### 4. Keeping both runtimes would leave two answers

Once Task Sessions could place, run, steer, resume, and deliver concrete work,
the old generic loop looked like a competing lifecycle. The branch removed it
rather than maintaining a bridge.

That deletion propagated widely: `lf loop`, `lfq`, daemon exec routes,
stacked queue state, `combine`, `next`, flowloop driver/pass/run, DTOs,
webhooks, docs, prompts, tests, and Swift shortcuts all moved.

This was the first scope jump. A task runtime became a replacement for the
system's general worker and delivery model.

### 5. Deleting the generic loop removed Project pursuit

The old `project` flow could repeat until its KRs held. After generic loops
were removed, a Project remained Linear data plus a one-pass skill sequence.
Nothing durably selected tasks, waited without spending tokens, woke on task
events, or resumed the same Project transcript.

The local continuation added `ProjectSession` to fill that hole. It also
generalized Task command storage into `child_commands`, introduced
`SessionSupervisor`, added Project and Task event ledgers, and built an
observation outbox for Task → Project → Wave delivery.

This was the second scope jump. Replacing one worker runtime created a new
agent tier, and that tier justified generic child-session infrastructure.

### 6. The client was adapted before the product model was settled

Rust now exposes distinct Project and Task Session snapshots. Swift maps both
back into its existing `Run` model. This made the data visible without first
deciding how Projects, Tasks, executions, attempts, transcripts, and terminal
sessions should appear to a human.

The branch is technically broad because each local consistency repair pulled
another surrounding layer into the new ontology.

## Major technical changes

### Durable Task Sessions

`task/mod.rs`, `task/runner.rs`, `ops/task.rs`, and `ops/task_pm.rs`
introduce:

- one persisted session per Linear issue;
- immutable launch receipts for issue, Project, Wave, PM snapshot, base commit,
  branch, worktree, agent, and provider;
- process generations that can die and resume without changing task identity;
- provider session reuse;
- explicit Task states from created through merged/abandoned;
- pending Linear writeback after code has already merged;
- one PR observed both from the runner and GitHub webhooks.

### Structured commands and receipts

Commands are persisted before provider delivery and move through
`persisted → claimed → accepted | failed | superseded`. The receipt records
whether input became a live steer, next turn, replacement, or decision.

Turn completion uses a transaction that either claims queued commands or makes
the process inactive. A command racing that boundary is handled by the current
generation or relaunches the same durable session.

The runner still polls every 200 ms while active, but correctness no longer
depends on a caller sleeping and hoping the poll wins.

### Durable Project Sessions

The local continuation adds one runtime per Linear Project:

- a provider transcript and process generations;
- a bounded pursuit iteration;
- supervision of Task Sessions in the same Project;
- waiting with no process while tasks run;
- wakeup from Task observations;
- a fingerprint intended to stop no-progress spinning;
- completion when every current Linear KR is marked as holding.

The Project process runs from the Wave home but is instructed not to edit it.
The boundary is prompt policy, not an execution sandbox.

### Supervision, events, and observation delivery

`SessionSupervisor` links a Task either directly to its Wave or to one
Project Session. The owning Wave remains an override authority.

Task and Project events are domain-specific. Consequential events also create
an outbox row in the same SQLite transaction. A Project consumes Task
observations before acknowledging them; a Wave journals child observations
idempotently before acknowledgement.

This is stronger than the short-lived agent bus and avoids copying raw child
tool chatter into the parent thread.

### Storage expansion

Migrations 062–067 add or evolve:

- `task_sessions`;
- Task events and commands;
- Project context, PM writeback, agent/provider identity, and command receipts;
- `project_sessions`;
- shared `child_commands`;
- `project_events`;
- `observation_outbox`;
- Task supervisor columns.

The later migration rewrites experimental `tc_` and `td_` identifiers into
`cc_` and `cd_`. That is appropriate recovery for this dogfood database,
but it is not a reason to carry the same migration history into a clean
extraction.

### Process and daemon changes

Process launch and tmux liveness helpers moved into shared engine/session
modules. The daemon no longer exposes the generic exec/queue path. PR-merged
webhooks locate owning Task Sessions, transition them to merged, and reconcile
Linear.

### Deletions

The branch removes:

- public generic `lf loop` execution;
- `lfq`;
- flowloop driver/pass/run;
- daemon exec routes and `lf_exec`;
- the in-process queue;
- `combine`, `next`, and stack queue delivery;
- stacked worktree controls and the Swift “next iteration” shortcut.

Some old concepts remain in historical migrations and the still-live generic
Run/terminal Session ledger. The deletion is therefore a product-surface
cutover, not yet a single internal data model.

### Client projection

`lf status --json` now includes Project and Task Sessions. Swift decodes
those snapshots but converts both to `Run`:

- a Task Session becomes `flow: "task"`;
- a Project Session becomes `flow: "project"`, with the repo as its worktree
  and `main` as its branch.

This preserves display compatibility by discarding domain distinctions. It is
the clearest evidence that the product data model remains unresolved.

## Major behavioral changes

| Before | Current branch | Working product direction |
|---|---|---|
| A generic worker can begin from free text | A file-writing Task must already exist in Linear before its worktree | Keep free-text `start`: create the Linear Task, then launch its worktree as one user operation |
| A generic loop can run any flow repeatedly | Waves plus domain-specific Project/Task runtimes repeat | Wave, Project, and Task are the only public loops |
| Worker identity is primarily a Run/worktree | Task identity is the Linear issue plus one Task Session | The Task owns the worktree and delivery; parallel Workers/Execs work inside it and own no worktrees |
| Waves, Projects, and workers may acquire placement implicitly | Project coordination currently runs from the Wave home; Tasks create sibling worktrees | Waves and Projects operate from the canonical `main` checkout; only a Task may allocate a worktree |
| Detached workers report only at pass boundaries | Waves can address, steer, interrupt, resume, and wait on Tasks | A Wave controls Projects and Tasks, never an unscoped Exec; every Task belongs to a Project |
| An authored loop bit decides whether another pass runs | Project and Task runners make some deterministic state checks after each provider turn | Each domain controller decides repeat, wait, block, or complete from authoritative state; no agent-authored loop bit |
| Projects are planning data executed by a generic loop | A Project can own a durable agent runtime that supervises Tasks | Project becomes one of the three loops; whether that requires the current `ProjectSession` representation remains open |
| Child reports are prose/bus messages | Consequential events travel as typed, idempotent observations | Preserve typed linkage, but choose the minimum events needed to supervise work |
| UI shows Runs and terminal Sessions | Backend adds Project/Task Sessions; UI flattens them into Runs | Humans create and talk to Waves; Project/Task hierarchy and activity are drill-down state, not competing chat destinations |

Record-first is an internal invariant, not a rejection of free text. The
formal lower-level API can still be:

```text
lf task start "add a hello-world command" --project <project>
  → create Linear Task
  → receive its stable id
  → create the Task worktree
  → start the Task loop
```

If Linear creation fails or is ambiguous, no worktree appears. Once the record
exists, `lf task run <id>` is the smaller formal primitive.

The normal human path is conversational, while the creation chain beneath it
is strict:

```text
Human
└── creates and talks to Wave
    └── creates/selects Project     must exist before a Task
        └── creates Task            must exist before a worktree
            └── Task worktree
                └── Worker / Exec
```

No command may skip a level by creating an anonymous worktree or an unowned
Task. Humans may inspect or directly control Projects and Tasks through the CLI
as an explicit operator surface, but they should not need to manually walk the
hierarchy for ordinary work.

The strongest behavior is the Task afterlife: review, CI repair, merge, and PM
writeback remain owned by the original task, worktree, and provider history.

The least settled behavior is not whether Projects repeat; the working model
says they do. It is whether the current mind-like `ProjectSession`, provider
transcript, tmux process, shared Wave-home checkout, and observation machinery
are the simplest way to implement that loop.

## Loop semantics: current code and working direction

There is no `/goal` runtime or command in this branch. `GOAL.md` is the Wave's
durable specification and context; the Wave server reads it into each turn.
The actual repetition lives in the controller around provider turns.

All three loops run one three-phase domain iteration, then a deterministic
transition:

| Loop | Body today | Transition after the body |
|---|---|---|
| Wave | Run `wave_clarify → wave_pursue → wave_mutate` | Yield after the full iteration; chat, cadence, or child activity wakes the next |
| Project | Run `project_clarify → project_pursue → project_mutate` | Complete if non-empty KRs hold; wait on active Tasks; block on an unchanged fingerprint; otherwise loop the full flow |
| Task | Run `task_clarify → task_pursue → task_mutate` | Mark merged/submitted from PR state; loop the full flow when the worktree changed; block rather than spin on no progress |

Each loop has the same three semantic phases:

```text
clarify the current intent → pursue it → judge the evidence
```

The owned artifact and completion rule change by tier:

| Loop | Clarify | Pursue | Judge |
|---|---|---|---|
| Wave | Keep the objective and portfolio computable | Create or direct Projects, with a direct Task only for a small concrete change | Decide what deserves attention now, then return to the scheduler; a Wave never completes |
| Project | Make the definition and KRs proof-shaped | Create, direct, and wait on Tasks | Compare current evidence to the KRs; repeat, wait, block, or complete |
| Task | Turn the directive into a concrete change design | Implement, test, review, and repair in the Task worktree | Compare the PR and verification evidence to the task; repeat, wait, or finish only on merge/abandonment |

This three-stage grammar belongs in the Loopflow language as the **body of one
domain iteration**, while the domain runner owns the lifecycle around it. The
runner sends each flow step through the same harness and provider session. The
language composes the three tier-specific skills; it must not regain a generic
LM-written loop bit:

```yaml
# the durable Task runner plays this whole flow through one harness
- task_clarify
- task_pursue
- task_mutate
```

Project and Wave use the same shape with their own skills. The existing
playhead is the phase cursor; do not add a parallel phase state machine.
Finishing a skill advances the playhead. Finishing the whole flow asks the
authoritative domain runner to choose repeat, wait, block, or complete. If it
chooses repeat, the runner loads another invocation of the same flow through
the same harness. The provider never decides its own lifecycle merely by
writing a file or returning a magic bit.

One durable domain session and provider transcript span all flow steps and
iterations. They are not three disposable agents. Project and Task runners
keep one harness active; the Wave resumes the same provider session across
phase processes. A Wave steer reaches the active step live when the adapter
supports it; otherwise it interrupts and restarts that same step in the same
transcript with the new directive. The playhead and directive version are
visible together, so the Wave can tell both *what the child is doing* and
*which instruction it is doing it under*.

The cleaner target is:

```text
Wave loop
│   runs on main; owns no worktree
└── Project loop                   runs on main; owns no worktree
    └── Task loop                  owns one mutable worktree and delivery lifecycle
        ├── Worker / Exec          bounded work inside the Task worktree
        └── Worker / Exec          may run in parallel inside the same Task
```

Each loop performs one full policy flow, then its controller chooses the next
state:

| Loop | Repeat | Wait | Complete |
|---|---|---|---|
| Wave | More judgment or coordination is immediately useful | Return to its scheduler until a human, cadence, or child event wakes it | Never; a Wave is a durable operating context |
| Project | Open KRs remain and some action is available | Tasks or external evidence must change first | Current KRs observably hold |
| Task | Implementation, review, or CI repair is actionable | Review, CI, a decision, or another external event is pending | PR merged or Task explicitly abandoned |

“Wave always goes back” should mean it returns to the scheduler, not that it
must busy-spin. The current zero-idle continuous playhead is a cadence policy,
not the definition of a Wave.

The working model therefore has one loop per concept, not necessarily one
skill per concept. Clarify, pursue, and mutate are explicit phases inside the
loop. The controller, not the LM, owns the transition after mutation. This
keeps the useful compositional part of the flow language while deleting the
part that made a generic flow pretend to own durable product lifecycle.

Workers/Execs are below the Task loop. They are execution attempts, not PM
objects, loops, worktree owners, or direct children of a Wave. A Wave starts or
steers a Project or Task; a Project starts or steers Tasks; a Task may use
parallel Workers. Independent isolated work must first become another Task.

## Main as the control plane

“Waves and Projects operate in main” means they use the canonical repository
checkout as a read-and-coordinate surface. They do not own branches,
worktrees, commits, or PRs. Linear mutations and Task lifecycle operations are
valid there; repository mutations are not.

Every public command needs an explicit main-safety decision:

| Command effect | From Wave or Project on main |
|---|---|
| Read repository, status, PM snapshot, events, or transcripts | Allow |
| Create or update a Linear Project under the current Wave | Allow |
| Create a Linear Task under an existing Project | Allow |
| Start or steer an existing Project or Task | Allow |
| Create the Task's sibling worktree | Allow only as part of Task start/run |
| Edit files, commit, push, open a code PR, or run a file-writing implementation flow | Refuse immediately and name the required Task command |
| Create an unowned Task, anonymous worktree, or raw top-level Exec | Refuse immediately and identify the missing Project or Task |

The Wave normally issues those Project and Task commands in response to human
conversation. The hard admission checks protect that conversational path from
silently turning into untracked repository work.

The failure should happen before provider launch or filesystem mutation. An
error should say which invariant failed and provide the shortest valid command,
for example:

```text
cannot edit from Project main context: create or select a Linear Task, then run
`lf task start "<change>" --project <project>`
```

The canonical main checkout must also remain clean. Fast-forwarding main is a
boundary operation between Wave/Project commands, never something that changes
the code beneath an active provider turn. If the checkout is dirty or not on
main, the next Wave/Project command fails loudly rather than resetting or
silently switching it.

## Steering is the core technical contract

A Wave's ability to direct a child is the product of five things:

```text
directability = authority × shared intent × delivery × observability × recovery
```

If any factor is weak, a large command surface does not help. A Wave that may
send `steer` but cannot see the Task's current brief or know whether the child
incorporated it is not meaningfully in control.

### What PR #872 already does well

The current command path is structurally strong:

```text
Wave or Project invokes `lf project/task …`
→ ambient ids prove the caller's authority
→ SQLite persists a typed command
→ an inactive child is relaunched
→ the current process generation claims the command
→ the runner injects, queues, or interrupts through the Harness
→ a durable receipt records the actual effect
→ typed child events travel through the observation outbox
```

- Every command has a stable id and source: Wave, Project, human, attachment,
  or system.
- A foreign Wave or unrelated Project is rejected before persistence.
- The owning Wave can control a Task even when its immediate supervisor is a
  Project.
- Commands survive process death and are reclaimed by a later generation.
- Turn-boundary settlement prevents a command from landing in the inactive
  gap.
- `follow-up`, `steer`, and `interrupt-with-replacement` have distinct intent.
- Provider behavior is reported honestly: Codex can live-steer; Claude and
  OpenCode interrupt and resume.
- A stopped nonterminal child resumes when a command arrives.

Those mechanics are worth preserving.

### Where directability is still weak

1. **Launch lacks an atomic delegation brief.** `project run <id>` and
   `task run <id>` start the first provider turn from the Linear record and
   ambient Wave context. The Wave cannot create the session and persist “why I
   am starting you, what to prioritize, and what not to do” in the same
   transaction. Sending a steer afterward races the initial turn.
2. **There is no folded current intent.** The event log remembers commands,
   but status cannot answer “what does the Wave currently expect this Project
   or Task to do?” after several replacements and follow-ups.
3. **`accepted` is transport-level.** It means the provider API accepted the
   input. It does not prove that the child incorporated directive version 4
   into its plan or abandoned the superseded direction.
4. **Authority and observation routing are conflated.** `supervisor` chooses
   the immediate outbox destination. The root Wave retains Task override
   authority, but Project-supervised Task events do not reach the Wave directly.
   The Wave can control descendants it may not know need control.
5. **Launch context is broad but causality is weak.** Prompt assembly can load
   Wave memory, recent Wave chat, scratch, the Linear record, and repository
   context. That is more context than a child needs in some places and less
   precision than it needs in the one important place: the parent's explicit
   delegation.
6. **The child is still modeled as one provider thread.** The emerging product
   says a Task may own several Workers. Wave commands should target Task intent,
   not whichever provider process happens to be active.
7. **The decision channel is not yet a product surface.** A Task or Project can
   persist a request and wait for an answer, but provider approvals remain
   auto-approved and Wave Chat does not render or answer child decisions.
8. **The UI erases the evidence.** Receipts, effects, supervisor links,
   decisions, current directives, and descendant state do not survive the
   Swift conversion to generic Runs.

### The session setup that maximizes Wave direction

Creating a Project or Task runtime should atomically establish a durable
control handle before any provider turn starts:

```text
ChildControl
  subject             Project id or Task id
  root_wave           permanent authority and visibility root
  parent              Wave→Project or Project→Task planning edge
  controller          immediate command/decision owner
  directive           current versioned delegation brief
  state               running / waiting / blocked / terminal + reason
  capabilities        steer, interrupt, resume, decide, inspect
  inbox_cursor        durable command delivery
  observation_cursor  durable parent visibility
```

This need not be a public generic Session type. It is the common control
contract implemented by the Project and Task loops.

The initial transaction should do all of the following or none of them:

1. Resolve the existing Linear Project or Task and owning Wave.
2. Persist the loop identity and root-Wave authority.
3. Persist an explicit delegation brief from the parent.
4. Capture the relevant work-record version and execution policy.
5. Register the observation route and decision owner.
6. Reserve one process generation.
7. Only then launch the provider.

The public shape can remain small:

```text
lf project run <project> --directive "pursue onboarding first"
lf task run <task> --directive "fix the parser before updating docs"
```

When a Wave creates the record from conversation, record creation and this
launch transaction compose into one user action.

### Two kinds of direction

The Wave needs to distinguish durable work changes from tactical execution
control:

- **Change the work:** update the Linear Project definition/KRs or Task
  description. The child receives a typed `WorkRevised` observation naming the
  new record version.
- **Change the execution:** follow up, steer, interrupt/replace, pause, resume,
  decide, or abandon without rewriting the planning record.

Without this split, important scope changes live only in a provider transcript;
with too much forced PM editing, ordinary steering becomes ceremonial.

### Version intent, not just messages

Every initial brief and replacement direction should advance a monotonically
increasing directive version. A follow-up attaches context without replacing
the brief. The child reports the newest version it has incorporated.

```text
persisted   Loopflow durably owns the command
applied     the runner injected, queued, or replaced with a known effect
incorporated the child crossed a boundary acknowledging the directive version
settled     the resulting state/plan is visible to the parent
```

The current persisted/claimed/accepted/failed/superseded states cover the
first two levels. The missing `incorporated_version` is what lets the Wave and
UI say “Task INF-123 is now working from your correction,” rather than merely
“the HTTP call returned 200.”

### Root authority, local supervision

Store the relationships separately:

- `wave_id`: permanent root owner; may inspect, steer, interrupt, resume, or
  answer any descendant;
- `project_id`: permanent planning parent for every Task;
- `controller`: the Project loop normally supervising the Task, or the Wave
  when it intentionally takes direct control;
- `created_by`: audit attribution, not authority.

Immediate Task events wake the Project. The Wave retains a queryable live index
of every descendant and receives only material attention events by default:
decision required, blocked, failed, PR opened, merged, or directive not
incorporated. It can drill into the full Task event stream without copying raw
worker chatter into its conversation.

### Wave commands target loops, not workers

The Wave directs a Project or Task. It never needs to know which Worker or
provider process is currently executing. The Task loop owns fan-out, write
coordination, replacement, and aggregation for its Workers. This preserves the
Wave's control when a provider dies, a Worker finishes, or the Task changes its
internal execution plan.

## The product model currently encoded

| Concept | Current responsibility | Durable identity | Execution surface |
|---|---|---|---|
| Wave | Human conversation, memory, cadence, project selection, root supervision | Wave id + repo-owned files + registry row | listener, resident, journal, provider playhead, Wave home |
| Project | Definition and KRs in Linear | Linear Project id | none by definition |
| Project Session | Repeated KR pursuit and Task supervision | `ps_` row + provider session id | replaceable tmux process in the Wave home |
| Task | Concrete work in Linear | Linear issue UUID / identifier | none until run |
| Task Session | Worktree, provider history, control, PR lifecycle, PM completion | `ts_` row | immutable worktree + replaceable tmux process |
| Run | Flow/skill execution and historical worktree/PR state | generic Run row and run-event lineage | varies |
| Session | Terminal/control process visible to clients | generic Session row | tmux or embedded client |
| Provider session | Vendor transcript continuity | vendor id stored on Project/Task Session | harness-specific |
| Process generation | One attempt to animate a Project/Task Session | generation number embedded in its row | PID + tmux name |

Three different things are called “session,” two things are called “run,” and
the closest existing representation of an execution attempt is a process
generation with no first-class cross-domain identity.

The intended product model is smaller:

| Product noun | Owner / parent | Repository placement | What can execute |
|---|---|---|---|
| Wave | created and addressed by a human | canonical `main`, no owned worktree | human conversation, its own coordination turns, and Project/Task commands |
| Project | exactly one Wave | canonical `main`, no owned worktree | its own KR-pursuit turns; Task commands |
| Task | exactly one Project | exactly one Task worktree | its implementation/review loop and task-scoped Workers |
| Worker / Exec | exactly one Task | the parent's Task worktree | one bounded, steerable undertaking; no child worktree |

Session, provider transcript, process, and attempt remain implementation
details until one of them earns a distinct human-facing behavior.

## Concrete implementation/design mismatches

These are not polish. They mark decisions that the branch text claims more
strongly than the runtime currently implements. The phase model and native
Swift hierarchy are resolved: each tier owns one
`*_clarify → *_pursue → *_mutate` flow, controllers own lifecycle, and Swift
preserves Project/Task identity instead of manufacturing Runs.

- Project and Task commands share a table and aliases, but still have separate
  Rust command structs and largely duplicated runner control loops. The second
  consumer has proven common mechanics exist; it has not yet found their
  simplest boundary.
- Project launch and relaunch enforce a clean canonical `main`, but the runtime
  does not yet prevent a Wave resident and Project process from taking
  overlapping turns in that checkout.
- The design allows standing-frontier Projects to wait indefinitely. The
  implementation blocks any Project whose fingerprint repeats with open KRs.
- The branch's normal tests cover many local state transitions, but the planned
  ten-scenario × three-provider conformance matrix and live two-Task Project
  path remain unexecuted.

## The loose thread: loop identity versus Task execution identity

The planning and loop nouns now have a coherent nesting:

- Wave: durable operating context and top-level loop;
- Project: measured bet and KR-pursuit loop inside one Wave;
- Task: concrete work loop inside one Project, owning one worktree and its
  delivery lifecycle;
- Worker/Exec: bounded execution inside one Task, potentially parallel, with
  no planning identity or worktree of its own.

That resolves the product hierarchy, but not yet the technical runtime model.
The branch currently gives Project and Task separate Session records, separate
runners, provider session ids, tmux processes, generations, command aliases,
and events. The UI then converts both back into generic Runs.

Do not solve that duplication by making Exec the generic public runtime for
all three loops. Wave and Project have domain turns on main; Task has domain
turns in its worktree and may additionally fan out bounded Workers. Their
provider control, receipts, and process-attempt machinery may share an internal
implementation without sharing a product identity.

The technical shape to test is:

```text
WaveLoop       → TurnAttempt on main
ProjectLoop    → TurnAttempt on main
TaskLoop       → TurnAttempt in Task worktree
               └── Worker / Exec* in the same Task worktree
                   └── ExecAttempt
```

Only the last two rows may write repository files. A Wave or Project never
exposes “start an arbitrary Exec”; it starts or steers the next domain object.

There is a real concurrency tension to resolve. Several Workers can reason,
inspect, test, or own explicitly partitioned edits in one Task worktree, but
uncoordinated concurrent file writes recreate the shared-worktree corruption
problem. “Parallel Workers share the Task worktree” therefore needs a write
discipline: one writer at a time, explicit file ownership, or a Task-level
integration mechanism. It should not be solved by silently giving Workers
their own worktrees, because that would reintroduce untaskified isolated work.

## Wave Chat is the end-to-end product surface

Humans create and talk to Waves. Project and Task controls exist so the Wave
can delegate reliably, not so the app can become a collection of peer chat
windows.

### What the app shows today

The macOS Wave detail already has the right broad frame: plan on the left,
Wave conversation on the right. Its data model has not caught up:

- the plan pane renders Linear Projects and KRs, then separately renders Open
  PRs, Active Sessions, and Backlog;
- Project and Task snapshots are converted into generic `Run` values;
- the chat transcript renders Wave turns and generic provider items;
- child observations are converted into message-like turns;
- the header exposes the old flow playhead, current/next/return steps, Skip,
  and arbitrary flow enqueue;
- only the Wave composer has controls; child directives, receipts, decisions,
  and ownership are invisible.

This UI describes the implementation that preceded the new hierarchy. It
cannot yet answer the questions a human asks while talking to a Wave:

- What did the Wave decide to pursue?
- Which Project and Task did it create?
- What is each child currently supposed to do?
- Did my correction reach it, and did it change direction?
- Who owns the next move?
- What needs my judgment?

### Keep one composer

Wave Chat remains the only normal human composer. A human writes:

```text
Prioritize the parser fix. Stop the docs task until that lands.
```

The Wave decides whether to revise Linear, steer a Project, steer or interrupt
a Task, or ask for clarification. The UI shows those control actions and their
receipts. It does not require the human to choose a child transcript first.

Selecting a Project or Task may offer “Tell Wave about this” and prefill an
addressed reference in the Wave composer. Direct child CLI controls remain an
operator escape hatch; a second child-chat composer is not part of the first
product.

### Replace the plan pane with a live work map

The left pane should render the native hierarchy rather than four unrelated
lists:

```text
Wave objective

Project: First-run onboarding                 running
  KR 1                                        holds
  KR 2                                        open
  current direction v3                        incorporated

  INF-123  Fix parser                         running
    direction v2                              incorporated
    next move                                 Task
    PR                                        —

  INF-124  Update docs                        paused
    direction v2                              applied, awaiting incorporation
    next move                                 Wave
```

Each row needs identity, state and reason, current directive version, whether
the child incorporated it, next-move ownership, and delivery state. Workers
stay summarized under their Task unless one fails or needs attention.

Selecting a row opens an inspector in the same pane or a drawer:

- Linear definition, KRs, or Task description;
- current delegation brief and revision history;
- recent material observations;
- command receipts and actual effects;
- provider/session continuity for diagnosis;
- Task worktree and PR where applicable;
- a transcript drill-down for debugging, not a competing primary conversation.

### Render delegation as structured conversation events

The Wave transcript should interleave ordinary conversation with compact,
linked cards for consequential actions:

```text
Wave created Project “First-run onboarding”
Wave started INF-123 · directive v1 persisted
INF-123 incorporated v1 · implementing parser fix
Wave redirected INF-123 · v2 applied as live steer
INF-123 incorporated v2 · docs deferred
INF-123 opened PR #912 · waiting for review
```

Decision requests become cards with options and lineage. The Wave should answer
routine child decisions itself. When it needs human judgment, it asks in the
same Wave conversation and the card makes the originating Project/Task visible.

Raw tool chatter remains in the child transcript. Wave Chat receives material
state changes, summaries, and attention—not every command execution.

### Snapshot plus motion

The existing data architecture is close to what the UI needs:

- `lf status <wave> --json` supplies a durable point-in-time hierarchy;
- the Wave SSE stream supplies live motion and transcript turns;
- SQLite and Linear remain authoritative; Swift owns no lifecycle state.

Change the status wire shape from parallel generic arrays into native domain
snapshots that preserve relationships:

```text
WaveDetailSnapshot
  wave
  projects[]
    planning state + loop state + current directive
    tasks[]
      work state + loop state + current directive + delivery + worker summary
  attention[]
```

Stream typed `ChildControlChanged`, `ChildStateChanged`,
`DecisionRequired`, and `DeliveryChanged` frames over the existing per-Wave
connection. On reconnect, the query snapshot repairs any missed live frames.
Do not make Swift replay the event log or infer domain state from chat prose.

## Unanswered product questions

The hierarchy is now firm; these choices still affect the implementation:

1. Is the current directive stored as a folded command-log value, a dedicated
   field, or a typed brief record with its own history?
2. What exact child boundary constitutes “incorporated”—the next provider turn
   starting, a structured child acknowledgement, or a new plan/state event?
3. Which descendant events reach the Wave as live attention, and which remain
   queryable without entering the conversation?
4. When a Wave overrides a Project's Task direction, does the Project receive
   the same directive event before it may issue another command?
5. Does a Project need one durable provider transcript across wakeups, or can
   each turn reconstruct itself from Linear, directives, and observations?
6. How do parallel Workers divide or serialize writes in one Task worktree?
7. Does every Task target one PR to `main`, or can research and operational
   Tasks complete with another typed outcome?
8. Who may mark a Project KR as holding, and what evidence is retained?
9. What is the standing-frontier behavior when open KRs remain and no change is
   healthy rather than blocked?
10. Should the child inspector expose any immediate destructive control, or
    should all normal interaction—including interrupts—flow through the Wave?
11. How should a committed Linear create recover when session or worktree
    launch fails afterward?
12. Which state wins when Linear, SQLite, provider transcript, process, git,
    and GitHub disagree?

## Coherent implementation path

The branch should now converge vertically instead of expanding another backend
layer or being split horizontally.

1. **Settle the native detail snapshot.** Define Wave → Project → Task DTOs,
   current directive versions, incorporation state, next-move ownership,
   delivery, and attention. Stop converting Project/Task state into Runs.
2. **Make delegation atomic.** Add an initial directive to Project/Task launch
   and persist it before provider start. Create-and-run composes the Linear
   mutation with that launch receipt without duplicating records on retry.
3. **Separate authority from routing.** Persist root Wave, planning parent,
   immediate controller, and creator independently. Give the Wave a complete
   descendant index while keeping immediate wakeups local.
4. **Complete the receipt contract.** Preserve persisted/applied effects and
   add directive incorporation/version reporting at a child boundary.
5. **Make observations serve the UI.** Publish material child-control and
   lifecycle frames on the existing Wave stream; repair from the detail
   snapshot after reconnect.
6. **Rebuild the Wave detail pane.** Replace generic sessions/backlog/PR lists
   with the live Project/Task work map and inspector. Keep one Wave composer.
7. **Render structured delegation.** Creation, direction, incorporation,
   decisions, blockers, and delivery appear as linked transcript cards.
8. **Finish decisions through Wave Chat.** Let Projects and Tasks ask their
   controller; let the Wave answer or escalate to the human with visible
   lineage.
9. **Align the loops.** Give Wave, Project, and Task one policy turn followed
   by deterministic repeat/wait/block/complete transitions. Remove stale loop
   bits and the old playhead/enqueue UX.
10. **Add task-scoped Workers only after the Task control surface holds.** The
    Task aggregates Worker state and owns write coordination; the Wave never
    addresses Workers directly.
11. **Dogfood the whole path.** One human conversation creates a Project and
    two Tasks, redirects one live, pauses the other, resolves one decision,
    survives process restarts, merges both PRs, verifies KRs, and leaves a
    legible Wave Chat history.

## Aggressive implementation session

### Mission

Turn this branch from a backend lifecycle experiment into a coherent product
slice centered on Wave Chat. Preserve the strong Task/Project command
mechanics, make delegation and incorporation explicit, and expose the native
hierarchy without adding another public runtime.

The implementation session is intentionally multi-commit. Each checkpoint
must compile and preserve migrations, but the session should keep moving until
the vertical demo works or a genuine external dependency blocks it.

### The demo

From Wave Chat, a human says:

```text
Make first-run onboarding self-explanatory. Fix the parser before the docs.
```

The Wave creates or selects the Linear Project, creates two Tasks, and launches
them with explicit delegation briefs. The work map appears immediately. The
human then says:

```text
Pause the docs task. Make the parser accept --hello too.
```

Wave Chat shows the docs Task pause and the parser Task advance from directive
v1 to v2: persisted, applied with the honest provider effect, then
incorporated. The same Task later shows its PR and merge without changing
identity or worktree.

### Non-negotiable constraints

- Humans create and talk to Waves; there is one normal composer.
- Waves and Projects run from clean canonical main and own no worktrees.
- Every Task belongs to a Project before it allocates one worktree.
- Workers belong to a Task and never allocate their own worktrees.
- A Wave controls Projects and Tasks, never raw Workers or arbitrary Execs.
- Linear owns Project/Task planning truth; SQLite owns runtime/control truth.
- Root Wave authority survives local Project supervision.
- Provider-specific steering stays behind Harness capabilities.
- No compatibility adapter may keep Project/Task-as-Run alive in Swift.
- Do not add remote execution, alternate delivery policies, rich Worker UI, or
  a generic session framework during this session.

### Data model to implement

Use one common directive ledger for the existing Project and Task loops:

```rust
string_id!(ChildDirectiveId, "dir_");

enum ChildRef {
    Project(ProjectSessionId),
    Task(TaskSessionId),
}

enum DirectiveKind {
    Initial,
    Replacement,
    WorkRevised,
}

struct ChildDirective {
    id: ChildDirectiveId,
    target: ChildRef,
    version: u32,
    kind: DirectiveKind,
    text: String,
    source: ChildCommandSource,
    command_id: Option<ChildCommandId>,
    issued_at: OffsetDateTime,
    applied_at: Option<OffsetDateTime>,
    incorporated_at: Option<OffsetDateTime>,
    incorporated_summary: Option<String>,
}
```

Add `current_directive_version` and `incorporated_directive_version` to Project
and Task runtime state. Store directive history in `child_directives`, unique
on `(target_kind, target_id, version)`. Initial launch inserts version 1 in the
same transaction as the session reservation. `steer` and interrupt with a
replacement create a new version atomically with their command. Follow-up,
bare interrupt, decision, resume, and abandon do not replace current intent.

Keep the existing durable command state, but expose its meaning precisely:

```rust
struct ChildControlReceipt {
    command_id: ChildCommandId,
    directive_version: Option<u32>,
    state: ChildCommandState,
    effect: Option<ChildCommandEffect>,
    incorporated: bool,
    generation: Option<u32>,
    accepted_at: Option<OffsetDateTime>,
    incorporated_at: Option<OffsetDateTime>,
    error: Option<String>,
}
```

Add a child-only acknowledgement operation, authorized by the ambient Project
or Task session id:

```text
lf project acknowledge <project> --directive <n> --summary "…"
lf task acknowledge <task> --directive <n> --summary "…"
```

The Project and Task policy prompts require this acknowledgement before
continuing from a new directive. A missing acknowledgement remains visibly
pending; Loopflow does not infer semantic incorporation from a successful
provider request.

Replace the Swift-facing generic arrays with a native detail projection:

```rust
struct WaveDetailSnapshot {
    wave: WaveSnapshot,
    loop_state: Option<String>,
    projects: Vec<ProjectDetailSnapshot>,
    attention: Vec<AttentionSnapshot>,
}

struct ProjectDetailSnapshot {
    project: PmProjectSummary,
    runtime: Option<ProjectRuntimeSnapshot>,
    directive: Option<DirectiveSnapshot>,
    next_move: NextMove,
    tasks: Vec<TaskDetailSnapshot>,
}

struct TaskDetailSnapshot {
    task: PmTaskSummary,
    runtime: Option<TaskRuntimeSnapshot>,
    directive: Option<DirectiveSnapshot>,
    next_move: NextMove,
    delivery: Option<TaskDeliverySnapshot>,
    workers: WorkerSummary,
}
```

Projects and Tasks without runtime sessions still appear. Build the hierarchy
from the cache-only PM snapshot joined to SQLite runtime rows. `NextMove` is a
typed owner plus reason: human, Wave, Project, Task, review, CI, or external.

### Checkpoint 0: establish the gate

Before implementation:

1. Run focused Rust and Swift tests covering current child control and Wave
   Chat contracts.
2. Record the baseline failures, including the known headless UI runner caveat.
3. Confirm migration 067 is the last schema migration and add one forward
   migration for this campaign rather than rewriting dogfood history in place.
4. Keep the one scratch file; do not recreate deleted notes.

Checkpoint only if unrelated dirty code appears. The current scratch changes
are intentional design work.

### Checkpoint 1: native hierarchy reaches Swift

Implement `WaveDetailSnapshot` end to end before changing visual design:

1. Join the PM snapshot, Project Sessions, Task Sessions, directives, delivery,
   and attention in the Rust `lf status` query.
2. Delete Project/Task `toRun` conversion in `RegistryQuery.swift`.
3. Add required Swift DTOs and shared round-trip fixtures.
4. Make `RegistryQuery.status` return native Wave detail plus any truly
   historical generic Runs separately; do not concatenate them.
5. Update `WaveDetailPane` to compile against a `WaveWorkMap` model even if its
   first rendering is plain.

Checkpoint commit: **`wave chat: preserve the Project and Task hierarchy`**.

Proof: a cache-only status query and Swift fixture show one Project with two
Tasks, their runtime states, controller links, PR, and next-move owners.

### Checkpoint 2: atomic delegation and versioned direction

Implement the directive ledger and launch contract:

1. Add the migration, store APIs, domain types, and DTO fixtures.
2. Extend `project run/start` and `task run/start` with an optional explicit
   directive; deterministically derive version 1 from the Linear record when
   the lower-level caller omits it.
3. Ensure Wave/Project skills always pass an explicit delegation brief.
4. Insert session, directive v1, controller/root ownership, and initial command
   before process launch.
5. Make `steer` and interrupt-with-replacement atomically insert vN+1 and
   supersede every unincorporated older directive/input.
6. Keep follow-up as contextual next-turn input without changing the directive.
7. Include the directive id/version, root Wave, planning parent, controller,
   constraints, and acknowledgement command in every Project/Task seed.

Checkpoint commit: **`child control: make delegation atomic and versioned`**.

Proof: crash tests at reservation, persistence, provider application, and
restart show exactly one current directive and no instruction loss or duplicate
Task/Project record.

### Checkpoint 3: incorporation and root-Wave visibility

Close the gap between sending and directing:

1. Implement `project/task acknowledge` with ambient-session authorization and
   monotonic version checks.
2. Record directive applied and incorporated events separately.
3. Extend receipt wait with `--until applied|incorporated`.
4. Keep immediate Task observations routed to its Project controller.
5. Also route material descendant events to the root Wave: directive changed,
   incorporation overdue/completed, decision escalation, blocked, failed, PR
   opened, merged, and abandoned.
6. Notify a Project when its Wave overrides one of its Tasks so it cannot
   unknowingly steer from stale intent.
7. Add a queryable root descendant index; the Wave never depends on receiving
   every raw Task event to discover its children.

Checkpoint commit: **`child control: prove directives were incorporated`**.

Proof: the owning Wave redirects a Project-supervised Task, the Project sees
the override, a foreign Wave is refused, and the Wave receipt reaches
incorporated after a process restart.

### Checkpoint 4: the Wave Chat work map

Make the new model visible before adding more runtime behavior:

1. Replace Objective/Projects/Open PRs/Active Sessions/Backlog with one native
   Project → Task outline.
2. Show KR proof, loop status and reason, directive version/state, next-move
   owner, Task PR, and worker summary.
3. Add selection and an inspector for current brief, recent material events,
   receipts, provider continuity, worktree, and PR.
4. Add “Tell Wave about this” to prefill a stable Project/Task reference in the
   sole Wave composer.
5. Keep direct destructive child controls out of the first UI. The CLI remains
   the operator escape hatch.
6. Delete fake Project/Task Run rows and the UI language “hands” where it now
   obscures the domain.

Checkpoint commit: **`wave chat: show delegated work as Projects and Tasks`**.

Proof: the Wave detail renders unstarted planning records, active children,
waiting/blocked reasons, directive incorporation, and delivery from one status
snapshot with no Linear network request.

### Checkpoint 5: structured delegation in the transcript

Connect durable child motion to the conversation:

1. Define one typed child-control activity payload shared by journal, wire,
   Rust fixtures, and Swift.
2. Emit it for creation, direction, incorporation, controller override,
   decision, blocker/failure, PR, merge, and abandonment.
3. Stream activity on the existing Wave SSE connection.
4. Render compact linked cards inline with Wave turns; selecting a card selects
   the same object in the work map.
5. On reconnect, replay durable activity or reconstruct the current cards from
   the Wave journal; never leave the UI dependent on a missed live frame.
6. Keep raw child provider items and tool chatter behind transcript drill-down.

Checkpoint commit: **`wave chat: render child direction and outcomes`**.

Proof: a steer issued by the Wave appears once with directive version, actual
provider effect, and later incorporation; reconnecting produces the same
visible history without duplication.

### Checkpoint 6: decision round trip

Complete the supervision loop already sketched in the backend:

1. Let Tasks request decisions from their Project controller and Projects
   resolve routine questions.
2. Let Projects escalate a linked decision to the Wave.
3. Let the Wave resolve it autonomously or ask the human in Wave Chat.
4. Render human-facing options as one decision card in the Wave conversation.
5. Continue the same Project/Task provider transcript after resolution.
6. Replace `AutoApprove` only for approvals that can be represented safely by
   this decision protocol; keep other provider permissions explicit rather
   than inventing a second approval system.

Checkpoint commit: **`wave chat: carry child decisions to the human`**.

Proof: Task → Project rejection/revision → Wave escalation → human answer →
same Task transcript continues, with duplicate answers idempotent and foreign
answers refused.

### Checkpoint 7: align the three loops and remove stale UI

Use the working product to simplify the old lifecycle:

1. Give Wave, Project, and Task one domain flow each, composed from clarify,
   pursue, and mutate skills.
2. Move repeat/wait/block/complete decisions into deterministic controllers.
3. Remove Project/Task loop-bit language; mutate judges evidence but never
   decides its controller's lifecycle.
4. Keep the active tier flow phase as useful status, while removing Skip and
   arbitrary flow enqueue from the ordinary Wave Chat product.
5. Enforce clean-main admission for Wave and Project commands; reject file
   mutation before provider launch with the exact Task creation instruction.
6. Keep Task worktree and PR lifetime unchanged through review and merge.

Checkpoint commit: **`loops: make Wave, Project, and Task the only runtimes`**.

Proof: `rg` finds no public generic loop, arbitrary flow enqueue, or agent-owned
loop bit; the three controllers pass deterministic transition tests.

### Stretch: Task-scoped Workers

Only start this if the Wave → Project → Task control demo is green.

Add Workers as Task-internal execution, sharing the Task worktree. Start with
one-writer-at-a-time scheduling; parallel Workers may research, inspect, or run
read-only checks while one Worker holds the mutation seat. The Task aggregates
their status and exposes only a summary to its Project and Wave.

Do not add Worker worktrees, direct Wave→Worker commands, or peer-to-peer Worker
chat.

### Test campaign

Run focused tests after every checkpoint and the full gate after checkpoints
4, 6, and 7:

```text
cargo fmt --check
cargo test -p loopflow child_
cargo test -p loopflow task_
cargo test -p loopflow project_
cargo test -p loopflow wave_
cargo clippy -p loopflow --all-targets -- -D warnings
swift test --package-path swift
uv run python scripts/test.py --rust --python --swift
```

Add one scripted-harness end-to-end test that runs the same delegation,
replacement, crash, acknowledgement, decision, and observation path for Codex,
Claude, and OpenCode capability profiles. Finish with the side-effecting live
dogfood described in the demo; record ids, directive versions, command effects,
provider session continuity, PRs, and final KR evidence here.

The known headless UI runner hang remains unproven rather than a regression.
All deterministic Swift tests and macOS build-for-testing must still pass.

### Cut line

The must-win session is checkpoints 1–5: native hierarchy, atomic directives,
incorporation, root visibility, and a Wave Chat experience that shows them.
Checkpoint 6 completes human judgment. Checkpoint 7 pays down the old lifecycle.
Workers are stretch.

If time compresses, cut Worker support, rich transcript drill-down, provider
approval mapping, and visual polish in that order. Do not cut the native data
model, atomic initial brief, directive incorporation, or the work map; those
are the product proof.

## Deferred: extraction and distillation

Do not build a distillation workflow or split PR #872 while this campaign is
active. The branch is serving as the integration surface for a coherent
vertical design. Reassess extraction only after the live Wave Chat path proves
which abstractions are real.

## Done when

- A human can create and direct all work through one Wave Chat composer.
- The Wave creates/selects a Project before creating each Task, and no Task
  worktree exists before its Linear identity.
- Wave and Project provider turns run from clean canonical main and fail before
  any repository mutation.
- Project and Task launch persist their explicit initial brief before the first
  provider input.
- A Wave steer returns a durable receipt naming directive version and actual
  provider effect, then visibly reaches incorporated state.
- Root Wave authority and visibility survive Project supervision, process
  death, and provider resume.
- Wave Chat shows the native Project → Task hierarchy, current direction,
  next-move owner, decisions, blockers, PR, and merge without fake Runs.
- Child activity appears once as structured linked cards and repairs correctly
  after reconnect.
- One decision travels Task → Project → Wave → human and returns to the same
  child transcript.
- Wave, Project, and Task are the only loops; Workers remain Task-internal.
- The deterministic Rust/Swift/DTO/conformance gates pass, and one live
  two-Task Project completes through PR merge and KR verification.

The first implementation move is checkpoint 1, not another Project Session
abstraction: make the native hierarchy cross Rust → JSON → Swift → Wave Chat,
then let every subsequent control change prove itself in that surface.

## Implementation checkpoint — 2026-07-13

The must-win slice now crosses the stack:

- `lf status --json` joins the cache-only PM snapshot to Project/Task runtime,
  directives, delivery, and next-move ownership as a native hierarchy. Swift
  no longer converts Project or Task Sessions into fake Runs.
- migration 068 adds the common directive ledger. Project/Task reservation
  persists directive v1 before provider launch; replacement steering advances
  it atomically; follow-up does not. Both loops expose child-authorized,
  monotonic acknowledgement and receipts can wait for application or explicit
  incorporation.
- root Wave visibility is independent of local Project supervision. Material
  descendant observations reach the Wave while transport-only persisted and
  claimed command chatter stays local.
- Wave Chat renders the Project → Task work map, current brief, incorporation,
  next move, delivery, and linked structured activity cards beside one Wave
  composer. Decision options prefill that composer instead of opening a child
  chat.
- Wave and Project processes launch only from a clean canonical `main`; every
  repository mutation still belongs to a Task worktree. Public generic loop,
  enqueue, skip, their HTTP doors, and their old Swift controls are gone.

The simulated architecture review changed the implementation in three places:
Project relaunches now re-check clean main at the process boundary, wave
discovery credentials are ignored so the listener cannot dirty main before its
resident starts, and the Wave receives command outcomes rather than every
transport transition. The cache-only native hierarchy and ignored discovery
files now have direct regression tests.

What remains outside this pass:

1. Checkpoint 6 still uses the existing durable decision backend. The UI makes
   options and lineage visible through Wave Chat, but provider approval mapping
   and a scripted Task → Project → Wave → human conformance test remain.
2. Checkpoint 7 is done. The public generic lifecycle and stale playhead UI
   were already gone. Wave, Project, and Task now each run a tier-specific
   clarify/pursue/mutate flow through the shared playhead, while their
   deterministic controllers own repeat/wait/block/complete. No phase writes
   a loop bit.
3. Migration 069 forward-repairs Project/Task Sessions created before the
   directive ledger. General runner writes preserve monotonic directive
   versions, and an unincorporated current directive blocks the flow boundary
   instead of disappearing into a waiting state.
4. The three-provider crash/steer/acknowledgement harness and live two-Task
   Linear/GitHub dogfood are side-effecting follow-ups; this headless pass did
   not create records, worktrees, pushes, or PRs.
5. Clean-main admission intentionally treats a compiled `MEMORY.md` update as
   a repository change. A memory checkpoint must be committed before a later
   Project process can launch; weakening that invariant would make the control
   plane silently writable again.

The known headless UI test-runner hang remains unproven rather than a
regression. Deterministic Rust, DTO, and Swift package gates are the evidence
for this pass.

## Current iteration: make the Wave flow yield

The harness owns execution of a tier flow; the tier controller owns whether
that flow runs again. One wake runs the complete Wave flow through the same
provider transcript:

```text
wave_clarify → wave_pursue → wave_mutate → idle
```

Completing `wave_mutate` means the Wave is done **for this wake**, not that the
durable Wave is complete. The resident must then wait for human chat, a typed
Project/Task observation, a cron, or the coarse safety heartbeat. It must not
start another paid iteration merely because a Wave has no terminal state.

Project and Task controllers remain different: they inspect authoritative KR,
Task, worktree, PR, and directive state after the full flow and immediately
repeat only when another iteration is concretely actionable. No tier uses an
LM-authored loop bit.

Implement this iteration in `flowloop/wave.rs`:

- restore the four-hour quiet-Wave heartbeat instead of the zero-delay
  continuous playlist;
- describe the live harness/provider transcript accurately;
- pin the production default and the one-wake/one-flow boundary in tests.

Done when an idle Wave consumes no provider turns between wakes, while a new
message, child observation, cron, or heartbeat still runs the complete Wave
flow and remains steerable during its active phase.
