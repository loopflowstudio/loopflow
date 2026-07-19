# Replace Feedback with Ask

## What to build

Remove Feedback as a runtime lifecycle. Run `demo` either as an interactive Mac
experience or as an ordinary headless skill whose `lf ask` command can block on a
durable Answer. Give the Project runner a detached answer lane so Task questions
do not occupy or steer its clarify/pursue/mutate conversation. Collapse the
control meaning of Launch into Run so one durable object owns execution
authority and its physical containment.

The intent is:

> "subscribed to questions on tasks the project owns, and spin up a quick llm
> agent to just answer that one question with the right context"

and:

> "keep answering questions out of the core project loop"

and:

> "do we need launch and run or could they be combined?"

## Current implementation slice

### Slice 1: Run owns execution

Implement only the execution-model reduction in this pass:

- move runner containment, cwd, and starting/live/stopping/ended transitions
  from control Launch onto Run;
- replace same-Run Launch replacement with a new Recovery Run when the runner
  containment changes;
- rename the trace hierarchy from AgentLaunch to AgentInvocation and preserve
  AgentInvocation -> Turn capture;
- relate invocations to a supervising Run for provenance without granting
  authority or using that relation for Interrupt/recovery target selection;
- migrate Rust and Swift DTOs and current schema with one implementation and no
  compatibility view or dual read/write.

Do not implement Ask, Answer, answer workers, runner lane changes, or interactive
demo in this slice. Feedback deletion belongs to the Ask slice. Attention or
handback fields may remain temporarily on AgentInvocation solely to preserve
current Feedback and interactive behavior, but they cannot carry Run
containment or authority. Record each temporary field in `scratch/questions.md`
and make its deletion an explicit dependency of Slice 2.

This slice is complete when focused store, recovery, trace, DTO fixture, and
Swift tests pass; Interrupt and recovery contain no latest-invocation query;
and the Slice 1 symbols listed under Code absence are gone.

## Execution model

Combine control Launch with Run. Keep the trace-level concept currently called
an agent Launch, but rename it AgentInvocation. This is not a successor to the
deleted Session object: it owns no Work lifecycle, durable authority, cadence,
or recovery policy. It is one concrete harness invocation recorded for trace.

```text
Work
└── Epoch
    └── Run                         one active authority and containment
        ├── AgentInvocation         core provider conversation
        │   └── Turn...
        └── AgentInvocation         one detached answer attempt
            └── Turn
```

```rust
struct Run {
    id: RunId,
    work: WorkRef,
    epoch_id: EpochId,
    home_id: HomeId,
    state: RunState,
    trigger: RunTrigger,
    retry_of: Option<RunId>,
    containment: Option<Containment>,
    cwd: Option<PathBuf>,
    created_at: Timestamp,
    started_at: Option<Timestamp>,
    ended_at: Option<Timestamp>,
}

struct AgentInvocation {
    id: AgentInvocationId,
    supervising_run_id: Option<RunId>,
    answer_ask_id: Option<AskId>,
    provider: String,
    model: Option<String>,
    account_id: Option<String>,
    surface: String,
    resume_token: Option<String>,
    started_at: Timestamp,
    ended_at: Option<Timestamp>,
}

struct Turn {
    id: TurnId,
    invocation_id: AgentInvocationId,
    basis: Basis,
    state: BoundaryState,
    // provider identity, output, and timestamps
}
```

Run is the scheduler claim, authenticated writer, and supervisor containment.
Its lease is the only execution authority. A Run may supervise several agent
invocations, but an AgentInvocation never gains authority merely by referring
to the Run. Core invocations receive the Run lease in their environment when
the skill must act as that Work; detached answer invocations do not.

Provider, model, account, resume token, surface, conversation capture, and
Turns belong to AgentInvocation. They may differ between the core and answer
lanes. The Run owns the tmux or process-group containment which contains both.
Interrupt and recovery target that containment directly, never whichever agent
invocation happens to have started most recently.

If only a provider conversation fails, the still-live runner may start a new
AgentInvocation in the same Run. If the runner containment itself must be
replaced, end the Run and reserve `RunTrigger::Recovery { prior_run_id }`.
There is one recovery representation; never rotate a lease to attach another
control Launch to the same Run.

`runs` enforces one non-ended Run per Epoch. Run state constrains containment:
Reserved has none; Active and Stopping have exactly one; Ended retains any
containment it acquired, while a never-started reservation may end without one.
AgentInvocation's `supervising_run_id` is provenance and supervision, not a
capability or a target-selection mechanism.

## Execution surfaces

`demo` is one skill with two execution surfaces.

### Interactive

When the flow reaches `demo`, the Mac app presents a live interactive
AgentInvocation. The background Run waits for that invocation's durable
handback while the User and agent conduct the interaction in the TUI. Explicit
successful handback advances the flow. Closing the window, losing the
invocation, or detaching cannot count as completion.

This is not a User-routed Ask. The TUI is the execution surface of the Demo
step itself.

### Headless

The same skill runs normally in batch mode. When it needs judgment from its
parent, its instructions tell it to run:

```sh
lf ask "Which behavior should this demo prove?"
```

`lf ask` persists the Ask, wakes the immediate parent, waits without consuming
model tokens, and prints the Answer to stdout. From Claude, Codex, Pi, and
OpenCode this is an ordinary long-running shell tool call; no provider-specific
tool injection or mid-turn message transport is required.

The execution surface and durable Work relationship choose the route. A
headless child with an immediate parent Work routes to that parent even when no
parent Run was live when the child AgentInvocation began; waking a stopped
parent is the point of the protocol. An interactive surface may route to its
authenticated User when it explicitly supports questions. A headless root with
neither route fails clearly rather than waiting for an absent mind.

## Durable exchange

```rust
struct AskExchange {
    id: AskId,
    turn_id: TurnId,
    route: AnswerRoute,
    question: String,
    asked_at: Timestamp,
    answer: Option<Answer>,
}

struct Answer {
    ask_id: AskId,
    author: Author,
    text: String,
    answered_at: Timestamp,
}
```

Persist one exchange row. The answer fields are nullable as one checked group;
the public `Answer` is a projection of the populated group. A partial unique
index on `turn_id` where the answer is null permits sequential questions but at
most one unanswered Ask per Turn. `UPDATE ... WHERE answered_at IS NULL` makes
the first authorized Answer win atomically. Repeating the same answer returns
the existing projection; a different second answer is a conflict.

Do not copy Work, Run, AgentInvocation, Epoch, or Basis onto the exchange. They
are already determined by
`Ask -> Turn -> AgentInvocation -> Run -> Epoch -> Work`.
This makes a question claiming one Work while pointing at another Work's Turn
unrepresentable. The stored route names only the answering authority: User, or
the immediate parent Work derived when the Ask is created.

Pending means the exchange has no Answer and its Turn and Epoch still permit an
answer. No Feedback aggregate, state enum, stored inbox, or queue row is needed.
`user_attention` and `child_attention` are queries over those facts.

Answer is not Steer. Steer is unsolicited durable direction which remains
available for delivery at a later Work boundary. Answer is targeted to one Ask
and returns through the blocked command in the current child Turn. Treating an
Answer as Steer would either replay an incorporated response or erase the
evidence that the current Turn received it.

Ask and Answer are Turn-local tool I/O and do not allocate Work epoch
revisions. Work Basis moves for durable direction such as Steer, not for a tool
call and its result already incorporated by that Turn. Recovery reads the
exchange directly. The unused `ToolResponseWrite` revision behavior is not
precedent for this exchange.

The child Run lease opens the Ask. The Answer route is stored, not supplied by
the caller:

- User route: authenticated User authority;
- Parent route: active Run authority for the immediate parent Work.

`lf work asks` lists pending routed Asks. `lf work answer <ask-id> <text>` is
the explicit manual/fallback response surface.

## Blocking command

`lf ask <question>` is idempotent for the current Turn:

1. create or recover that Turn's Ask;
2. after commit, call the idempotent parent wake operation;
3. poll for its Answer;
4. retry the wake periodically while unanswered;
5. print the Answer and exit successfully.

If a shell timeout kills the command, the Ask remains. `lf ask wait [<ask-id>]`
recovers the current Turn's exchange and returns an already-recorded Answer or
continues waiting. There is no Ask deadline; harness and Run recovery own dead
processes.

Intentional lifecycle control is different from process loss. Interrupting the
child Turn cancels its unanswered Ask without synthesizing an Answer; abandoning
the Work makes every Ask in that terminal Epoch historical and unanswerable.
Unexpected containment or harness loss preserves the exchange for recovery.
Attention projects only Asks which are still answerable under those lifecycle
facts.

Work remains `Running` while its live Turn is blocked in `lf ask`. Do not add a
`WaitingOnAsk` WorkStatus variant: the active Run and containment still exist.
Status and UI views may project the pending Ask beside WorkStatus when they need
to explain what the running Work is waiting for.

## Project runner

The Project process owns two lanes under one active Project Run:

```text
Project Run
├── core lane:   clarify -> pursue -> mutate
└── answer lane: oldest unanswered child Ask -> one-shot answer agent
```

The core lane remains serial and retains the Project provider conversation. The
answer lane has at most one fresh, ephemeral agent. It never advances or
interrupts the Project playhead and never reuses the core provider conversation.
The core invocation therefore contains only playhead Turns; child servicing
cannot pollute or accidentally advance it.

The answer agent receives only the context needed to speak as this Project:

- exact Ask and child identity;
- Project definition and KRs;
- relevant Wave goal and memory;
- Task directive, current evidence, and recent trace;
- current Project boundary, including direction from above.

Include the asking Task's prior Ask/Answer exchanges from its current Epoch
under the normal context budget. This preserves useful follow-up continuity
without replaying an unbounded history or depending on the parent core
invocation's memory.

Its instruction is to answer this question and return. It gets read-only tools
when investigation is necessary, but no Project Run lease. The runner captures
the final text and commits the Answer with its own lease.

The answerer is an AgentInvocation supervised by the Project Run and correlated
to the Ask through `answer_ask_id`. It has no Run lease. Starting it cannot
change Run state, become the interrupt target, or affect recovery fencing. No
`core | answer` control lane is needed on Run; the Ask relation already says
why the invocation exists.

A failed or killed answer agent leaves the Ask unanswered. Retry with capped
backoff rather than immediately re-ensuring it in a token-burning loop. After
repeated failures, keep the Ask pending and surface the latest failure as
attention until provider capability changes or an explicit retry is requested.
Answer-attempt trace is the durable evidence used to recover the failure count.

### Scheduling loop

```rust
loop {
    let inputs = project_inputs();

    apply_run_control(inputs.control);       // interrupt or abandon immediately
    ensure_answer_worker(inputs.oldest_ask); // independent of the core lane
    deliver_steers(inputs.steers);           // live when supported, durable otherwise
    update_observations(inputs.observations);
    ensure_core_turn();                      // current clarify/pursue/mutate step

    if project_run_can_end() {
        finish_run();
        return;
    }

    select! {
        control_tick => continue,
        core_event => settle_core_turn(),
        answer_event => settle_answer_attempt(),
        supervision_tick => supervise_tasks(),
        terminal_input => persist_user_control(),
    }
}
```

At core `TurnCompleted`, the playhead advances from clarify to pursue to mutate.
After mutate, authoritative Project and Task state decides Continue, Wait, or
Done exactly as today. Every boundary reconciles inputs before starting the
next step.

An Ask does not wait for such a boundary. A live runner sees it through the
same short store poll and starts the answer worker beside the core turn. If the
Project is stopped, `lf ask` calls `wake_project`; the replacement Run services
pending Asks before deciding whether core work is actionable. If provider
capacity cannot run both lanes, the Ask takes the slot: interrupt the core flow
body, answer, then retry that core step.

When the answer worker finishes, the runner writes Answer and drops the worker.
The child command returns. Follow-up is another Ask, not a servicing interval.

An unanswered child Ask prevents the Project Run from ending. When the core
disposition is Wait only because owned Tasks are still running, stop the core
AgentInvocation and retain the cheap Run supervisor in answer-only mode until
those Tasks settle. This is the low-latency path: a later Ask does not need a
scheduler or a new Project model invocation. External `wake_project` remains
recovery for a Run that ended for another reason. Done requires no answerable
child work; terminal control makes unresolved exchanges inert before the Run
closes.

### Direction from above

User or Wave direction remains Steer and targets only the core lane. While a
core Turn is active, `send_current` may deliver it immediately on a capable
provider. Otherwise it remains durable and enters at the next model boundary.
Only explicit Interrupt requires cancellation; provider steering capability is
never part of the semantic contract.

## Runner hierarchy

The same split continues through the ownership tree:

```text
User direction ───────Steer──────> Wave core
Project Ask ──────────Answer─────> Wave answer lane

Wave direction ───────Steer──────> Project core
Task Ask ─────────────Answer─────> Project answer lane

Project direction ────Steer──────> Task core
```

Steer always changes the owned child's work. Answer always resolves one exact
question from that child. Neither is translated into the other.

### Wave runner

The Wave gets the same two-lane shape as the Project. Its core lane owns user
chat, cadence, project selection, and Wave-level work. Its answer lane handles
Project Asks with a fresh one-shot agent using Wave goal, memory, current
boundary, the asking Project's definition and KRs, and the exact question.

Today child attention is turned into a Wave core pass, and an arriving child
item may be sent into or preempt that pass. Delete that path. Project Asks must
not become Wave chat input or occupy the Wave's core provider conversation.

The current Wave runner awaits a whole `run_pass` before returning to its outer
inbox loop. To make the lanes genuinely concurrent, the top-level supervisor
must own both jobs: start a core pass as a child job, start an answer attempt as
a separate child job, and select over events from each. Do not hide the answer
lane inside the core pass; that would stop answers whenever the Wave core is
waiting on its own model or User Ask.

A resident Wave sees new Project Asks through its short attention poll. A
stopped Wave is woken idempotently before the asking command waits. The woken
Run services pending Asks before deciding whether a core pass is actionable.

### Task runner

A Task has no owned child Work, so it needs no answer lane. Its runner becomes
simpler:

- delete Feedback routing, preemption, and Continue handling;
- run clarify, pursue, mutate, and headless demo as ordinary serial Turns;
- keep polling Run control and Steer while a shell call is blocked in `lf ask`;
- advance the playhead only when that Turn completes, never when the Ask opens;
- launch interactive demo as an interactive body and wait for its durable
  successful handback instead of starting a batch harness.

Normally the live harness owns the blocked `lf ask` process and resumes the
same Turn when stdout receives the Answer. After unexpected harness loss, the
replacement runner does not spend a model call rediscovering the wait: an
unanswered Ask keeps that step parked; an existing Answer is included when the
same step is restarted. Intentional Interrupt and Abandon use the cancellation
rules above.

### Shared answer mechanics

Wave and Project should share the small mechanism that supervises one answer
attempt, not a generic runner framework. Each parent builds its own domain
context and commits with its own Run authority. The answer agent has no parent
Run lease and cannot call `lf ask` in v1; it must answer from the supplied
context and read-only investigation or return a visible failure for retry. This
prevents a detached response lane from creating an invisible chain of blocked
parents.

## Failure and recovery

- Project runner dies: recovery observes the Run containment directly. Its
  process group contains the core and answer invocations, so proven absence
  ends the Run and incomplete invocation traces. Unanswered Asks remain
  queryable and a Recovery Run selects them again.
- Answer agent dies: no Answer is written; retry with a fresh agent.
- Child shell tool times out: rerun `lf ask wait`; the exchange is unchanged.
- Child Turn is intentionally interrupted: its unanswered Ask becomes
  unanswerable history; no denial or empty Answer is invented.
- Child Work is abandoned: its terminal Epoch makes every unresolved Ask inert.
- Child core AgentInvocation dies before Answer: the flow step remains
  incomplete. The same Run may start a replacement invocation; after runner
  loss, a Recovery Run does. Either waits on an unanswered Ask or seeds an
  existing Answer into the replacement Turn.
- Duplicate wake or answer attempts: one active Project Run serializes answer
  workers, and the first Answer write wins idempotently.

## Deletion

Execution ownership:

- control `Launch`, `LaunchId`, `LaunchState`, and `LaunchRoute` DTOs;
- `RunAdvance::LaunchStarting`, `LaunchLive`, and `LaunchEnded`;
- `AdvanceReceipt::Launch` and Launch handback as Run control;
- `control_launch_for_run`, `current_launch`, `current_launch_for_run`, and
  `launches_for_run`;
- `rotate_run_lease` and same-Run successor-Launch recovery;
- latest-Launch selection in Interrupt and observed-Launch fencing in
  `recover_run`;
- `idx_agent_launches_one_control_live`;
- `product_run_id`, `home_id`, `launch_state`, control containment,
  `opaque_basis`, attention, and handback columns from the trace table;
- `agent_launches` as a table and DTO name, replacing it with
  `agent_invocations` whose Run relation is supervision only;
- any new `Session` domain type or lifecycle; the already-deleted Project and
  Task Session model stays deleted;
- Swift control-Launch models and every UI assumption that the latest agent
  invocation is the current execution owner.

Feedback servicing:

- `feedback: true`, `Skill.feedback`, and `FlowPosition.feedback`;
- `FeedbackReviewer` and per-phase reviewer flags;
- `Feedback`, `ChildFeedback`, and `UserFeedback`;
- Launch attention columns and their re-arm logic;
- `route_feedback`, `feedback`, `continue_feedback`, and special runner turns;
- `control_turn_active`, `background_preempted`, `pending_child`,
  `delivered_child`, and the child `send_current`-or-interrupt path;
- `lf work feedback` and `lf work continue`;
- Feedback-specific Swift presentation and state;
- review steps whose only purpose was to create a Feedback checkpoint.

The standard flow may collapse elaboration plus review where they are one act.
A standalone review skill can remain a direct operation; it owns no lifecycle.

## Done when

### Data makes invalid states impossible

- SQLite permits at most one non-ended Run for an Epoch. A second reservation
  fails atomically rather than relying on runner polling.
- A Reserved Run has no containment. An Active or Stopping Run has one complete
  containment identity. An Ended Run cannot lose or partially overwrite a
  containment it previously owned.
- There is no control Launch foreign key to select, order, or become stale.
  Run authority and Run containment identify the same durable row.
- The current schema contains no Project, Task, or Agent Session table. Old
  migration text may describe the removed schema, but no runtime type, query,
  DTO, or compatibility view can recreate it.
- Starting any number of AgentInvocations cannot change Run state, rotate its
  lease, become its Interrupt target, or make recovery report that the Run
  "advanced" behind an observer.
- An AgentInvocation's optional Run relation grants no authority. Only
  possession of the opaque current Run lease permits writes as that Work.
- Every Turn belongs to exactly one AgentInvocation. Every Ask belongs to
  exactly one Turn. Work, Epoch, Run, AgentInvocation, and Basis are derived
  through those foreign keys rather than copied onto the Ask.
- It is impossible to persist an Ask whose claimed Work or Basis disagrees with
  its Turn because no such duplicate columns exist.
- The exchange check constraint permits either no answer fields or one complete
  Answer; partial answers cannot be stored.
- A partial unique index makes two simultaneous unanswered Asks for one Turn
  impossible while permitting a later follow-up after the first is answered.
- The answer update is first-writer-wins. A second different answer cannot
  overwrite evidence; an identical retry returns the recorded Answer.
- Attention is a query over the exchange plus Turn and Epoch lifecycle. An Ask
  from an interrupted Turn or abandoned Epoch cannot accept an Answer or remain
  actionable attention.

### Execution and recovery

- Interrupt marks the Run and stops its containment. It never queries for the
  latest AgentInvocation, so a concurrently starting answer invocation cannot
  divert an interrupt away from the core runner.
- Proven containment absence ends the observed Run directly. Unprovable or
  present containment keeps that exact Run fenced; no AgentInvocation ordering
  is involved.
- Replacing a dead runner always creates a new Recovery Run linked by
  `retry_of`. There is no path which rotates the old Run lease and attaches a
  second control process.
- Restarting only a provider conversation creates a new AgentInvocation under
  the still-live Run and does not acquire another scheduler slot or execution
  authority.
- Core and answer AgentInvocations can overlap under one Run and remain
  separately traced. Neither one's completion can settle the other's Turn or
  advance the other's playhead.
- Detached answer invocations receive no Run lease in their environment.
  Calling `lf ask`, mutating parent Work, or answering their own Ask fails
  authority validation.
- A runner crash kills both core and answer processes through the single Run
  containment. A Recovery Run re-derives unanswered work from the store.

### Ask behavior

- Interactive `demo` opens in the Mac app; its runner waits for explicit
  handback and resumes the flow afterward.
- Closing, detaching, or crashing the interactive AgentInvocation cannot
  produce a successful handback or advance the flow.
- A headless Task running under Claude, Codex, Pi, or OpenCode blocks in
  `lf ask`, receives a Project-authored Answer, and continues the same Turn.
- Opening or answering an Ask cannot move Work Basis or advance a flow
  playhead. Only the enclosing Turn's completion or explicit interactive
  handback can advance the step.
- An Answer cannot enter the Steer queue, and a Steer cannot satisfy an Ask.
  Recorded Answers therefore cannot replay at a later boundary.
- A Project can continue a core flow turn while its detached answer agent
  answers a Task, subject only to provider capacity.
- A Wave can continue its core pass while its detached answer agent answers a
  Project; Project questions never become Wave core passes.
- A Task runner contains no child-answer machinery and advances only when its
  actual flow Turn or interactive handback completes.
- Above-Project Steer reaches the live core Turn when supported and otherwise
  appears at the next boundary without loss or replay errors.
- A stopped Project is woken by an Ask and answers without first running a full
  clarify/pursue/mutate iteration.
- A Project with running Tasks and no actionable core work stops its core
  AgentInvocation but retains an answer-only Run supervisor. A later Task Ask
  does not wait for a new scheduler lease or Project core model call.
- `project_run_can_end` is false while an answerable child Ask exists. Done or
  Abandon makes every remaining child exchange terminal before ending the Run.
- User, sibling, and stale Run authority cannot answer a Parent Ask.
- Shell, answer-agent, AgentInvocation, and Run recovery preserve the exchange
  and do not advance an incomplete flow step.
- Repeated answer-agent failures back off from durable attempt evidence and
  eventually surface attention; a pathological Ask cannot cause an unbounded
  model-spend loop.
- No Feedback type, flag, attention column, Continue operation, or servicing
  loop remains.
- Ask, Answer, answer attempt, route, and incorporation are visible in
  `lf trace`.

### Code absence

These searches return no production-code matches:

```sh
rg 'control_launch_for_run|current_launch_for_run|rotate_run_lease' rust swift
rg 'RunAdvance::Launch|AdvanceReceipt::Launch' rust swift
rg 'product_run_id|launch_state|idx_agent_launches_one_control_live' rust swift
rg 'route_feedback|continue_feedback|ChildFeedback|UserFeedback' rust swift
rg 'control_turn_active|background_preempted|pending_child|delivered_child' rust swift
```

`agent_launches` is migrated once to `agent_invocations`; no compatibility
view, dual write, old/new DTO pair, or fallback query remains.

### Proof

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test -p loopflow store
cargo test -p loopflow trace
cargo test -p loopflow project::runner
cargo test -p loopflow task::runner
swift test --package-path swift -Xswiftc -gnone
```

The store suite proves the constraints and recovery transitions above. Runner
tests prove simultaneous core/answer invocations, answer-only idle, and
playhead isolation. Trace and Swift fixtures round-trip the renamed Run,
AgentInvocation, Turn, Ask, and Answer wire shapes with no defaults or legacy
fields.
