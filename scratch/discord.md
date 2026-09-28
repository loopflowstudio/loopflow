> Execution context: LOO-332. Primary design: `scratch/task-automation.md`.
> Source: `/Users/jack/src/loopflow.discord/scratch/discord.md` at `613a66ca8fd99b81b92a902c986dfc1540d6ca10`.
> Stacked on LOO-298 at `d07e569330c8dedceb5dd238ce9b22a7b6137006`. This destination owns ongoing edits.
> Inherited LOO-298 scratch is dependency evidence, not this Task’s assignment.

# Scheduled Task, Wave and repository operation

2026-09-28. Accepted product direction from the design conversation with Jack
Heart; implementation choices remain marked as proposals. Jack invoked
launch-plan, selected Product ownership, and requested a base on LOO-298.
LOO-298 retains ownership of its data-model and daemon deletion work.

## Execution decision

This is an additive series. The single-threaded core is defined fully in
[Task automation through verified landing](task-automation.md): finite Task
admission, watcher-free landing, CI/rebase repair, closure and desktop-managed
scheduling ship together. Keep that core in one Product Task/PR.

[Recursive Wave operation and repository VSM](recursive-vsm.md) is independently
useful through manual finite invocations and can start now under Product. It
does not implement the core's scheduling or delivery contracts.

Named follow-ups, intentionally not filed until the core contract settles:

1. **Discord channels and Task input continuity.** Bind one channel per
   Wave/subwave and one thread per Task; bounded read/write, durable catch-up,
   safe reply retries, and Discord/Linear input to all Task-registered
   non-interactive Sessions. Integrate active input and idle admission against
   the settled Session/Flow contract. Include provisioning/archive/transfer
   choices and the delayed-message/recipient-restart proof below.
2. **Scheduled Wave and repository operation.** Extend desktop schedule
   management to the manually usable Wave/VSM operations after the core's
   schedule lifecycle is proven; integrate connected chat after follow-up 1.
   Choose cadences/budgets and prove app-closed operation, independent Task
   progress and no duplicate governance pass.

The focused Discord chatbot remains outside this series. No new planning
manifest, marker or scheduler state is created by launch-plan.

## Intent and accepted direction

Jack: "i wanted to remove any notion of wave listener or wave resident and
refocus on only the decentralized model".

Jack proposed "cronning task-operate for every task, project-operate for every
project, etc", then corrected the operating objects: "yes, waves instead of
projects in fact".

Jack: "project is just part of a wave".

Jack: "then allowing that operate scirpt to see whether discord was connected,
and read/write from chat".

Jack: "we might later build a more hyperfocused discord chat bot experience but
i think thats different and doesnt need to be part of what we build here".

Build scheduled, finite Task, Wave and repository VSM operations so each can make progress
without an always-running Wave process. A Project is part of its Wave: the Wave
operation reads and updates that plan as part of its own work. It has no separate
operator or schedule. An operate pass can discover a Discord connection and
read/write its chat. Connection does not require a Wave listener or resident.

Scope is scheduled Task/Wave/repository operation with optional chat access. A dedicated
Discord chatbot experience is explicitly outside this design: no instant-reply
loop, always-connected bot process, or conversational product is required.
That is a possible separate future effort, not a promised follow-up increment.

Placement: Product, explicitly selected by Jack. LOO-298 is the stack dependency,
not this work's Task identity. This source checkout has no bound Task. Its design
was checkpointed at `ce0263277`; a direct rebase tried to replay unrelated main
commits #1307/#1308 and was aborted. Use prepared Product Tasks stacked directly
on LOO-298 and stage actual designs there before launch. Those destinations own
their working designs; this document remains source provenance after transfer.

## How this repairs Discord after LOO-298

The proposed integration replaces the old listener's responsibilities with
finite operations and durable evidence:

| Responsibility | Proposed owner |
| --- | --- |
| Wake work when nothing is running | OS schedules managed through the desktop and shared `lf` operations |
| Read connected chat and decide what it means | Wave operation reads its channel; Task execution reads its Task thread |
| Keep continuity over unknown gaps | Operator/channel progress plus unresolved decisions, not a live conversation process |
| Execute a Task | Its selected Flow, admitted/recovered by task-operate |
| Repair CI, rebase, and close after merge | Finite scheduled Task delivery checks using existing domain operations |
| Respond in Discord | Explicit chat write from an operate pass, backed by delivery evidence |

An end-to-end example: a Discord message arrives while the app and operators are
closed. The next Wave tick reads it with enough prior context, evaluates it
through the Wave's S1–S5 responsibilities, and responds or updates the appropriate
Task/plan within its authority. Task operation admits the selected Flow when
idle and not awaiting human input. Landing hands off and exits; later ticks
repair failed/overdue CI or a required rebase, then settle the Task after merge.
A later Wave pass can report the outcome once in chat. Existing Task work never
depends on a successful Discord read or a Wave pass running first.

This restores useful asynchronous chat without restoring lfd, a Wave resident,
or a bot that reacts continuously. It requires a bounded chat read/write path
and persistent progress; the in-memory startup-skipping bridge observed in
LOO-298 does not satisfy that path. Wave channels and Task threads give messages
an operating scope. Handling a message and resolving
the concern it raised remain separate facts.

The scheduler is shared infrastructure, not a single agent responsible for all
work. Task admission is mechanical; wave-operate and repository vsm-operate each
run at their own cadence with their own scope. Extending desktop schedule
management to those two passes is the proposed integration; Jack explicitly
requested desktop-managed Task scheduling so far. The focused Discord bot
experience remains outside this work.

Proof must include delayed chat catch-up, crash/retry without duplicate replies,
busy or human-waiting Tasks remaining untouched, post-exit CI/rebase repair,
and eventual Task closure. Merely starting a new Run for each message is not
the intended experience.

## Proposed responsibility boundary

- Task operation: determine the right Flow, run it through the existing Flow
  machinery, and follow progress and recovery through successful landing.
  Preserve intended steps, review gates, worktree and execution history.
- Wave operation: read its objective, memory, current plan and Task outcomes;
  choose useful planning, prioritization or coordination actions; exit.
- VSM operation: one repository-wide pass across its Waves, scheduled alongside
  Task and Wave operations. Examine the whole and take useful authorized actions.
- The OS scheduler supplies wakeups. Wave activity is not a prerequisite for
  Task activity; VSM activity is not a prerequisite for either. No operator
  waits indefinitely for its next wakeup.
- Existing Flow state owns execution order and review gates. Scheduled operation
  must not introduce a second cursor or infer approval from elapsed time.

## Discord within an operate pass

Accepted capability: discover whether Discord is connected, then read and write
chat from the operation itself. Proposed interaction:

1. Read the object's current state and available chat binding.
2. If connected, fetch relevant messages with author, message ID and source link.
3. Decide what deserves a response or work alongside the other operating inputs.
4. Perform authorized actions and post any warranted reply through the connected
   chat capability. Record the source and delivery outcome, then exit.

No connection is an ordinary state: proceed with other work. A configured but
failing connection is a visible read/write failure, not evidence of an empty
channel. Routine passes should not generate chat noise.

Discord remains the transcript source. Proposed durable bookkeeping remembers
which input was handled and whether a reply was delivered, using existing
execution records where sufficient; it must not recreate the Wave journal or
resident inbox. Reading a message alone does not prove it was handled. Exact
checkpoint and retry semantics remain to design.

The chat capability owns credentials fetched through Doppler; raw bot tokens
must not enter prompts or logs. Chat input retains its actual author and does
not itself grant permission to bypass Task review or execution ownership.

### Channel per Wave, thread per Task

Jack: "I think channel per wave (or subwave) and thread per task sounds good".

Accepted conversation topology: each Wave or subwave has its own Discord
channel; each Task has a thread in its owning Wave's channel. A subwave uses the
same channel contract as any Wave. Bind channels/threads to durable object IDs
rather than relying on names; renaming a Task must not split its conversation.

Proposed reading/writing boundaries:

- wave-operate reads its channel for direction, planning, coordination and
  identity. Task results and escalations appear as concise summaries with links
  to their threads; raw Task transcripts are not automatic Wave context.
- Task operation/execution reads its own thread for direction, blockers and
  delivery discussion, and replies there. It does not independently answer the
  whole Wave channel. Recover relevant parent messages when a thread needs them.
- Each binding retains independent handled progress and unresolved concerns.
  Repository VSM reads Wave evidence and follows relevant links as needed.

### Task-wide input to non-interactive Sessions

Jack: "thread messages should be like comments on linear and automatically get
injected into the context of all the (non-interactive) sessions registred as
that task".

Accepted delivery scope is Task membership, not ownership of the managed Flow.
Discord thread messages and Linear comments use the same Task input path. Every
registered non-interactive Session for that Task receives the input, including
independent helper/research Sessions. Interactive Sessions do not receive this
automatic injection. Delivery grants no additional execution ownership and does
not resolve human Sessions or review gates.

Proposed mechanics: retain one source event with provider/message identity,
author, content and source link, then record delivery separately for each
recipient Session. A shared source cursor cannot substitute for recipient
progress: one Session receiving a message must not consume it for the others.
Active Sessions poll/import through their existing input-refresh path, including
when no task-operate pass is running. New or resumed non-interactive Sessions
receive applicable Task input in their initial context, then newer input while
running. Repeated fetches and handoffs must not inject the same event twice into
the same conversation. Provider limitations may delay injection to the next
supported input boundary; do not claim instantaneous delivery.

Context injection and public chat replies are separate actions. Broadcasting
input must not automatically post every recipient's final answer to the Task
thread. Preserve source identity across any Discord/Linear relay and distinguish
recorded outbound echoes from new input, so retries do not form feedback loops.
Exact recipient progress storage, edits and late Task binding remain to design
against LOO-298's final Session model.

Proof: two non-interactive Sessions on the same Task both receive a new Discord
message and a Linear comment; an interactive Session and a different Task do
not. Repeat source polling, restart one recipient, and create a later Session:
input remains available to intended recipients without duplicate injection into
an existing conversation. None of this launches a competing Task Flow.

Channel/thread provisioning, archive/reopen behavior and Task transfer between
Waves remain to specify. The accepted topology is not permission to provision
channels during this design conversation.

## Task operation through landing

Jack: "task-operate is just like basically figure out what teh right flow is
to run and run it, make sure things get to landed successfully (but dont skip
intended steps etc)".

Accepted role: select and run the appropriate Flow, then ensure its intended
work reaches successful landing. The Flow owns execution order and review
boundaries. task-operate uses that machinery rather than implementing its steps
again. It does not require a separate S1–S5 assessment.

Proposed pass behavior:

1. Read Task intent, explicit Flow selection, existing execution and new input.
2. Choose an appropriate Flow if none is selected. Honor an explicit selection;
   do not choose anew every tick.
3. Start it or inspect its existing invocation. Let active work continue; recover
   interrupted work through existing resume/retry operations.
4. Address failures and pending review through the intended Flow path. A waiting
   review is not permission to skip ahead; a fresh tick is not approval.
5. Verify landing from actual PR state, preserving explicit manual-merge and
   other delivery constraints. A green build, finished Run or published PR is
   not proof of landing.

Keep unresolved blockers and the next action visible across ticks. The existing
Flow execution authority stays intact.

### Cron admission before task-operate

Jack proposed frequent cron ticks with a filter that skips task-operate when an
active Session or managed Task Flow is already running. If neither is running,
task-operate launches the appropriate Task Flow. Jack explicitly adds: "if
there's open human sessio nbut no active task flow, dont start one".

Proposed admission order, performed mechanically before starting an agent:

1. A completed/canceled Task has no further automatic work. Intentional holds
   and not-yet-selected backlog retain their meaning; exact eligibility is open.
2. An unresolved human Session for this Task means wait, even if no Flow or
   provider process is running. A closed terminal does not resolve the Session.
3. An active Task Session, Task Flow execution, or task-operate pass means skip.
4. Otherwise admit one task-operate pass. It selects/starts the right Flow or
   recovers the selected invocation, then exits after handing execution over.

Scope the check to the Task; an unrelated Session elsewhere does not stop it.
A persisted Flow that is parked or interrupted is not necessarily active
execution. Unknown execution state is not evidence that the Task is idle.
Use the existing execution evidence and recovery path to distinguish these.

The initial filter saves agent work; existing claims must also prevent two
overlapping ticks from starting competing operators or Flow drivers. Recheck
pending human review at admission so a newly opened review is respected.
Reuse existing claims where they can express this; exact admission mechanics
depend on LOO-298's final Session/Flow model. No parallel cursor or resident.

After explicit human completion, a later eligible tick can continue through the
Flow's intended transition. Cron wakeup, operator launch, and Flow launch are
three separate events; a busy/waiting tick does not need an agent invocation.

## Bounded landing and scheduled CI repair

Jack: "i want pr land to be able to END and then maybe we use similar logic to
launch ci-fix basically if PR is not green and the task is not closed then
launch ci-fix".

Direction: landing must not require a continuously running watcher.
Jack made the replacement explicit: "instead of trying to have pr land -c
esablish a watcher". `-c` records completion-on-merge intent; it does not create
a watcher or keep the invoking process alive. The desktop-managed scheduled
check later observes merge and applies that intent.

Proposed contract: `lf pr land` prepares/publishes the intended head, requests the
authorized merge disposition, records the pending delivery, and returns. A
later scheduled Task check observes GitHub and admits `ci-fix` when required.
Returning successfully means delivery was handed off, not that merge occurred.
The Task stays open until its required delivery is actually satisfied.

Apply Task admission first: no competing active Session, Flow, repair or pending
human review. Then inspect the current PR and current published head:

| Evidence | Scheduled action |
| --- | --- |
| Required checks pending, or observation unavailable | Wait; unavailable evidence stays visible |
| Actionable failed checks on the current head | Admit one `ci-fix` Run in the Task worktree |
| PR needs rebase to proceed, even with green CI | Admit `ci-fix`; rebase, resolve conflicts, verify and republish |
| Checks green, no rebase needed, merge still pending | Observe or take the already-authorized delivery action; no CI repair |
| Review or manual merge required | Wait for that action; never treat it as a CI failure |
| Authoritative merge | Apply the saved completion/PR-chain disposition |
| PR closed without merge | Surface the disposition; do not infer successful delivery |

### CI deadline

Jack: "probably need a timeout on CI too. maybe 30min?".

Proposed default: 30 minutes from the published head entering CI wait, persisted
across ticks and process exits. Include queued checks and expected checks that
never start; a fresh observation does not reset the clock. Failures can trigger
repair immediately without waiting for this deadline. Unknown provider state
remains an observation failure rather than proof that checks are stuck.

When required checks remain unresolved past the deadline, make the timeout an
actionable incident. Subject to the usual Task admission rules, `ci-fix` can
diagnose stuck/absent CI, repair an actual cause or request an appropriate rerun.
Timeout alone does not prove a code bug, authorize merge or close the Task.
Green CI waiting for review/merge is outside this CI deadline.

Record the attempted timeout repair so subsequent ticks do not repeatedly launch
it against unchanged evidence. A new published head or an explicit CI rerun is
a new attempt with its own timing; preserve prior incidents. Bound repeated
rerun/timeout recovery and surface a persistent external blocker. Exact retry
budget remains open. A sleeping machine handles the elapsed deadline on its
next tick; no timer process is required.

Proof: pending CI below 30 minutes launches no repair; overdue pending/missing
checks admit one diagnosis; repeated unchanged ticks do not admit duplicates;
explicit new attempts have fresh timing without deleting prior evidence.

`ci-fix` repairs, verifies and publishes, then requests the preserved merge
disposition and returns. The next tick considers the resulting current head.
Do not restart the whole feature Flow for post-delivery CI, skip earlier intended
steps because a PR exists, launch concurrent repairs, or repeatedly launch an
agent for the same unresolved external blocker without new evidence.

Jack clarified the green-but-unmerged case: "if needs rebase, dfinitely launch
ci-fix and expect ci-fix to run rebase". Needs-rebase is an immediate repair
trigger independent of CI failure or the 30-minute deadline. `ci-fix` already
begins with `lf rebase`; preserve that contract, including conflict resolution,
appropriate verification, publication and the saved merge disposition. Green
checks alone do not establish that delivery can proceed. Distinguish a concrete
rebase requirement from pending review, normal merge-queue waiting, or unavailable
mergeability evidence. Reobserve the current PR before acting on stale evidence.

Existing source: `ops/pr_landing.rs` contains the long-running supervisor,
pending/failing observations, incident tracking and merge settlement. Existing
`ci-fix` already ends after `lf pr arm`; its instructions expect the watcher to
resume. Move observation/repair admission/settlement into finite scheduled
operations and update that handoff. The one-shot `arm` behavior is a reusable
mechanism; exact CLI consolidation remains open. Do not retain two supervisors.

Flow semantics need explicit proof: after the landing command hands off, no
active driver remains merely to watch CI, no downstream step requiring an actual
merge is released prematurely, and later ticks settle only from authoritative
merge evidence. Preserve `-c`, bare landing and PR-chain intent across restarts.
Taskless landing behavior remains a separate scope question.

### Task closure is part of the scheduled lifecycle

Jack: "right, we would need to make sure we have task closing setup as well".

Accepted scope includes closing Tasks after verified delivery, not merely
launching CI repair. The scheduled check must keep pending completion discoverable
after the Flow and landing command have ended. Once GitHub confirms merge,
apply the saved `-c` intent through the existing Task settlement operation,
including its local and Linear updates. Bare landing keeps the Task open;
PR-chain intent continues the chain. A closed unmerged PR is not completion.

The current watcher calls `ops/task::settle_task_landing` from its merged branch
in `ops/pr_landing.rs`. Reuse that domain operation from finite scheduled work;
do not implement separate desktop or agent Task-closing logic. Closure needs no
new agent Run. Record merged-but-closure-pending distinctly when settlement fails,
retain it for retry, and show the exact failure. Repeated ticks must converge
without repeating completed effects or restarting the Task's implementation.

Required proof: `pr land -c` exits, all initiating processes stop, GitHub later
reports merge, and a scheduled tick closes the intended Task. Interrupt between
local and provider settlement and show the next tick finishes correctly. A
repeated tick is harmless; bare landing and closed-unmerged PRs do not close
the Task. No required Flow step or review is implicitly completed by this path.

## Desktop-managed scheduling

Jack: "then i think that's the kind of cron that i would expect the desktop
app to install / manage somewehre".

Accepted product owner: the desktop app installs/manages the scheduled Task
admission described above. Proposed mechanism: the app invokes shared `lf`
operations to install/update/remove macOS launchd jobs; launchd invokes a finite
`lf` command on schedule. The command evaluates eligibility and admits work.
The app exposes enable/disable, cadence and last outcome/error, without owning
a second scheduler or execution policy. Exact UI remains to design.

Installed work continues with the app closed, subject to the machine/user's
availability. A later wake reconciles current state; missed intervals do not
mean replaying every missed tick. Schedule changes and repeated app launches
must converge on one installed obligation rather than duplicate jobs.

Existing source already installs cron jobs under `~/Library/LaunchAgents`
through `ops/cron.rs`, with CLI management in `lf/commands/ops/mod.rs`.
No cron/launchd management references were found in either checkout's
`swift/Loopflow` Swift sources on 2026-09-28. Extend shared Rust scheduling
operations and expose them through the desktop's normal `lf` path.

Proposed simplification: one finite Task-admission sweep per repository can
discover eligible Tasks on each tick, instead of installing/removing an OS job
for every Task. Either arrangement must retain independent per-Task claims.
This mechanism, support for additional platforms/Homes, and desktop management
of Wave/VSM cadences remain open. No actual job installation is part of this
design conversation.

## Repository-wide vsm-operate

Jack clarified: "so we have task and wave-operate and then we could also have a
parallel vsm-operate", then "i mean vsm-operate would be like just one for the
whole repo".

Accepted scope: one VSM operation for the repository, alongside per-Task and
per-Wave operations. This supersedes the tentative interpretation of a universal
Flow invoked separately for each Wave. It replaces neither task-operate nor
wave-operate and introduces no Project operator or resident process.

Proposed thinking draws on `engine/builtins/wave/goal/s1.md` through `s5.md`
and the existing S2–S5 scan/assess skills:

- S1 / delivery: are the Waves delivering useful outcomes independently?
- S2 / coordination: where do Waves collide or need shared boundaries clarified?
- S3 / present operation: is capacity serving the repository's outcomes, and
  where is effort stalled or wasted?
- S4 / adaptation: what changed outside the current plans that demands attention?
- S5 / identity: do the Waves and proposed adaptations still serve the
  repository's purpose? Hold the tension between present commitments and future
  needs; escalate changes beyond the operation's authority.

### Identity through the graph

Jack corrected the implied allocation of identity to the repository pass:
"wait no, looking over again, i think its important that identity e.g be
something being determined through the whole graph".

Jack then clarified: "so vsm is meant to be recursive".

Jack made the Wave scope explicit: "So yeah, i guess Wave gets s1-s5".

Wave operation therefore includes all five functions: delivery through its
Tasks (S1), coordination among them (S2), current capacity and effectiveness
(S3), sensing and adapting to change (S4), and determining its purpose and
boundaries (S5). These are responsibilities within wave-operate; this does not
require five separate agents, processes or scheduled jobs. The repository pass
applies the same reasoning across Waves. Task operation has the narrower
Flow-selection and delivery role described above.

Accepted direction: VSM is recursive, with S1–S5 explicitly within each Wave.
Repository operation considers the same functions across Waves. The operating
scopes do not partition the five functions into
exclusive owners. Identity is determined through the graph, not exclusively
by repository vsm-operate. Exact behavior at each scope remains to design.

Proposed application: a Wave applies the five functions across its Tasks and
responsibility; the repository does so across its Waves and shared purpose.
This implies a shared VSM thinking framework with scope-specific context and
actions, not five processes or five mandatory reports at every level. It does
not require persisting new child objects beneath every Task.

Proposed behavior: Task work can expose a mismatch in its purpose or its Wave's
assumptions; wave-operate can use that evidence to reconsider its objective,
boundaries or plan; repository vsm-operate can reconcile tensions across Waves.
Repository direction informs Wave and Task judgment in return. A lower-scope
finding can challenge a higher-scope premise, not merely be checked against it.
Repository synthesis participates in that process without monopolizing it.

Preserve evidence and unresolved disagreement at the relevant owners. A finding
does not silently rewrite another object's objective. Mechanisms for carrying
proposed identity changes, deciding them and returning decisions through the
graph remain open; do not introduce a mandatory approval chain for ordinary
Task progress. The repository-wide pass remains the latest scheduling direction,
but its earlier framing as the exclusive home for VSM thinking is superseded.

Each pass orients across these questions, investigates what changed, and takes
the one or two useful moves. No five-report ritual on every tick. Persist dated
evidence and unresolved questions so a later pass can reconsider them even when
there is no new chat. Silence in chat does not establish health or relevance.

## Chat continuity across irregular wakes

Jack observed that the interval between Runs is unknown, so the operator must
choose its chat context intelligently. Proposed mechanism, not yet an accepted
storage design: retain a per-operator/channel handled position across Runs;
fetch newer messages and selectively retrieve earlier thread context. Advance
only across messages durably considered, including deliberate no-action outcomes.
Fetching alone cannot advance it. Budgeted catch-up leaves unhandled input
pending; pending questions survive the handled position. Retry after a crash
must reconcile recorded actions/replies before repeating effects. No fixed
lookback window substitutes for this progress record.

## Current system and dated observations

Inspected locally on 2026-09-28:

- This checkout's `rust/loopflow/src/ops/cron.rs` binds each schedule to a Wave
  plus a skill/Flow, generates launchd jobs, and records cron receipts. Its
  schedule parser accepts only a fixed daily time. Scheduling every Task and
  supporting shorter intervals therefore requires changes.
- LOO-298's working checkout has removed the Wave listener/resident and lfd
  stack. Its `controller/task/mod.rs` still delegates Task execution to the
  shared Flow driver and polls input while a step runs. Removing daemon
  ownership does not itself make that driver a scheduled single-pass operation.
- Its `lf/commands/discord.rs` provides an independent foreground polling
  bridge: one bounded Run per incoming message, an in-memory cursor, and startup
  that skips existing history. It does not yet provide durable scheduled input
  consumption. Whether to replace that bridge belongs to this design discussion.

Source provenance: `/Users/jack/src/loopflow.data-model-one-table-per/`, notably
`scratch/data-model-one-table-per.md`, `scratch/chapters.md`, and
`scratch/cutover/cut-i-delete-listeners.md`. These are observations of ongoing
work, not a claim that its entire branch is accepted or shipped.

## Demo and proof to refine

With no lfd or Wave resident, a scheduled Task makes progress while its Wave
is inactive. A separate Wave tick sees the resulting evidence and takes a useful
planning action. Repeated or overlapping ticks cannot create competing Task
drivers. A Task waiting for review stays waiting until explicit review arrives.
Completed or canceled Tasks do not restart because their schedule fires.
A repository VSM pass can identify a cross-Wave issue from their evidence;
Task and Wave progress continues when no VSM pass is running.

Discord proof: an input arriving while no operation is running is
read on a later tick, with a source reference and visible delivery outcome.
A fresh Run that silently loses earlier input does not count. Disconnected
operation still progresses; configured connection failures remain distinguishable
from no new input. No continuous polling bridge is required for this path.

## Remaining implementation decisions

The two selected designs resolve execution scope and defaults for their workers.
`operation-questions.md` distinguishes those choices from deferred chat details. Reuse
LOO-298's final execution records and claims. The launch-plan invocation
authorizes Task filing/preparation and execution through the selected Flows;
it does not install jobs or provision Discord in this conversation.
