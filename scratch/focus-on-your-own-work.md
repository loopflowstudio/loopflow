# Task conversation and ordinary Flows — design and state

October 5, 2026. LOO-353, Product, PR #1439. Written after Jack Heart's
delivery review of `e10e2add4`; the pass he then requested is built. The
reviewed evidence and Jack's exact words are in
`42c31408a:scratch/demo-task-flows.md`; open choices are in
[questions.md](questions.md); the code walkthrough of the reviewed revision
(before this pass) is [pr-review.html](pr-review.html). The pass's step-by-step
plan is at `67cd68157:scratch/focus-on-your-own-work.md`; the FlowSession
architecture it removed is at `6f246fda4:scratch/focus-on-your-own-work.md`.

## Flow model — October 5, governs the rest of this document

Jack Heart's statements are quoted in [questions.md](questions.md) under
"Step invocation". Where a later section describes steps carrying a
`FlowStep` payload or Flows found by searching command text, this replaces it.

Two different things were both being called a Flow:

| | TaskWorkflow (outer, second pass) | One `lf` flow run (inner, first pass) |
| --- | --- | --- |
| Shape | a start node, conversation stages, an end; edges are `lf` flows | implement, compress, decide, publish |
| Lifetime | the Task's; many processes and conversations | one driver process |
| Record | mutable, own identity, replace/take over like a Session | **FlowExec**: append-only, written by its driver |

**Order of work.** FlowExec and oblivious steps first; then TaskWorkflow, from
[task-workflow.md](task-workflow.md). Jack: "I think you own TaskWorkflow.
We cant do this design correctly without it."

- **FlowExec.** One row per Flow run, keyed by its driver Exec: Flow name and
  the graph as compiled at launch. One step row per step the driver starts:
  child Exec, graph key, iteration. Jack: "something more similar to an Exec
  but specifically for Flows"; "the FlowSession is mutable, but the Flow exec
  is not". Running, finished and each step's result are read from the Execs,
  never stored twice. The name FlowExec is the agent's choice.
- **Uniform.** Every Flow run gets one, ad hoc or started by `lf task run`.
  None is primary for a Task; no Task column points at one; membership is the
  driver's directory.
- **The driver maintains it;** clients (`lf flow list/show`, `task status`,
  Desktop) read it and work for any Flow. No API changes a running Flow; the
  control is ending its driver.
- **Steps are oblivious.** The driver starts each step as the plain command,
  a child process: `lf -b skill <name> [message]` or the operation's own
  command. No `FlowStep` payload, `--__flow-step`, `__flow-step` command or
  position variable. A step never reads or writes FlowExec. A deciding or
  routing step gets its answer contract in its message; the driver validates
  it and corrects it by resuming the same conversation.
- **Gone:** FlowSession as the record of an `lf` flow run, worker claims and
  generations, automatic recovery, resume, the pending-review pointer, the
  `replaced` state, steps reading a cursor.

**TaskWorkflow** is built as [task-workflow.md](task-workflow.md) describes,
unreviewed by Jack; his October 5 request to build every slice placed it in
PR #1439. It replaces the earlier "no runtime row" rule and the Task-primary
Flow idea.

## Outcome

One native Task conversation holds design and review, files, drafts, shells and
layout. Headless work is the ordinary `lf -b` command run in the Task's
worktree. Running a Flow leaves nothing behind beyond what `lf` logs for any
command.

## Decisions by Jack Heart

October 4 (unchanged):

- Interactive Task design and review stay in the ongoing Task conversation;
  more conversations may be opened deliberately. Headless work goes through
  ordinary `lf -b`; substantial implementation in the conversation is
  discouraged, incidental edits allowed.
- Product says **Session** for interactive and **Run** for headless
  AgentSessions; identity, history and attribution survive mode changes.
- **Waiting** is the one attention state; `--waiting` replaces `--needs-me`;
  `--interactive` stays a mode filter. Provider-specific signals and false
  positives are acceptable. Claude: binary stream-json only.
- Delete all Task-worker machinery, mutable Flow switching and automatic
  database-backed recovery. The caller owns recovery.
- Delete `lf session ready`/`complete` and the review handshake; no renamed
  replacement. Conversational feedback chooses what runs next.
- Task membership is owning Home plus resolved checkout plus explicit binds,
  excluding repository/Wave scopes. Hiding or selecting never terminates or
  approves.
- Approved design: conversation-stage workflows with operational-Flow edges;
  operational Flows hold autonomous loops and XORs only.

October 5 (delivery review):

- The cut "should have been DELETING the task worker APIs and routing more
  things through the basic (e.g. flow -b) apis." Task status presentation is
  "not the current focus of the task."
- "-b should continue to print the output and block just like it used to."
  `--mode`: "Definitely take this back."
- "running somethign from a task's worktree and passing --task should be
  equivalent ... we shouldnt get different codepaths or validations for
  either."
- "`lf task run` or whatever is allowed to do #1 and #3 and then go into the
  lf run flow": place the Task; fill defaults. On launch-time PR preparation
  (#2): "im not sure why we need 2."
- "there shouldnt be anything extra on top of what you get when you run
  `lf flow ...` in a taks worktree. we should still have good records of whats
  flows are running and at what stages just via normal logging from lf binary.
  I dont think we need the FlowSession datatype." Then: "fold it into this pr."
- `lf commit`: "should always push i guess ... or else should accept -p."

## Branch state after the FlowExec pass

FlowExec, oblivious steps, the Task entry and their readers are built
(`aa2f42289`, `70e76a7d3`); Jack Heart has not reviewed them. The itemized
account is at `74a738f72:scratch/focus-on-your-own-work.md` under this
heading. Takeover and resume are not built.

## Remaining from this pass

- **Unproven.** A real provider step, in particular whether real providers
  return the contract's JSON without a schema request; a driver killed
  mid-agent-step; the migration against a populated store; Desktop Start
  against a real `lf`; a cron-fired release Flow through the plain command.
  The real-Codex e2e (`tests/e2e/codex_connect.py --flow-decision-retry`) is
  ported to the message contract and FlowExec rows but not run.
- **A step can go unrecorded.** The driver learns a child's Exec by polling
  for its newest child every 10 ms. A driver killed between the child
  registering and that poll leaves the step's Exec without a step row.
- **Publish after an unobserved merge** still reaches GitHub through
  `pr publish`; `lf pr reconcile` remains its owner.
- **Failing before this pass, unchanged** (reproduced on `8535598ef`):
  `status_tests previous_release_merge_request_migrates_into_readable_status_and_roadmap`.

## Slices — one looping Flow builds these in order

Jack Heart, October 5: "i want you to manage a looping lf flow that implemetns
(and compresses, and realigns, etc.) all the slices". One `pursue` run loops:
each pass builds the first slice not marked done, compresses, syncs and
realigns; realign marks the slice **done** here with its commit and one-line
check result; the deciding step iterates while any slice is not done and
advances to publish only when all are; the list is the authority on how many there are. A proof that needs a person, a display
or a live provider is listed under the slice as a demo item and never keeps a
slice open. A pass that cannot finish its slice records why and stops blocked.

Flow history below means FlowExec rows joined to their Execs.

Slices 1–8 are **done**; each one's done-when, commit, check result and demo
items are at `74a738f72:scratch/focus-on-your-own-work.md`.

1. TaskWorkflow — `1f665b901`. 2. Loops (`loop:`, `loop-or-next`) —
`1793a4ed0`. 3. Review repairs (`--steers-after`, `lf task run ISSUE end`) —
`b30b193a0`. 4. Task primary — `3d416a655`. 5. Waiting — `e3ab80c55`.
6. Desktop — `f0b4196c1`. 7. Defaults and editing — `f87d02872`. 8. Docs and
website — `ce7648eed`.

Demo items from those slices, for Jack and not for the loop: a real provider
run of an edge and a live conversation reading stage guidance; a live
provider asking a question; Desktop on screen (rendering, Start, a Task
opening on its primary, progress moving, the Wave page's menu and Edit); the
website as rendered.

9. **Workflow as live state.** — **done**, `5cf71ffd7`, compressed
   `f57abb655`. Jack: "lets just call TaskWorkflow Workflow"; October 6:
   "mutable is probably wrong word. live state or something". Built to the
   contract in [task-workflow.md](task-workflow.md): position stored and
   written by the process running the edge; an append-only history of moves
   with who and a note; `lf task move` sets a stage and runs nothing; a
   stopped edge holds the Task on the edge. Compression removed authored
   edge names, as slice 10 asks. Check, October 6 at `f57abb655`: `cargo
   test -p loopflow --test task_flow_launch_tests` 6 passed (set, stopped
   edge retried then back, late landing corrected); status JSON and the
   Swift decoder by `dto_fixtures` 19 and Swift 34, as recorded at
   compression. Choices: [questions.md](questions.md). Demo items: the Move
   to menu and a stopped edge on screen; a conversation choosing an edge.

9a. **One Task run, many Flow execs.** — **done**, `274a63a8a`, compressed
    `24ba54689`. Words: Jack, October 6, "i think maybe cleaner to say run
    for task and exec for flow", then "er run for workflow api and exec for
    flow datamodel": *run* is the Workflow API's verb, *exec* is the Flow's
    record. A *Task run* is one `lf task run` carrying one edge; a *Flow
    exec* is one execution of a Flow, the FlowExec record. Say "Flow exec",
    not "Flow run", in status, Desktop and docs; the Desktop log in slice 11
    is the Flow exec log. (Collision carried to LOO-386: "Run" is also the
    product word for a headless conversation.) Jack, October 6: "lf task run
    should in addition to finding the flow also have some amount of retry,
    i.e. we shoudl allow task run has many flow runs". Built: `lf task run`
    carries the edge and starts `lf --task ISSUE run FLOW` as a child, again
    when a Flow exec fails; the position names the Task-run Exec and each
    attempt is its own FlowExec beneath it. Agent's defaults, not Jack's, in
    [questions.md](questions.md): three attempts; `lf run` exits 3 when
    blocked or stopped short, which is not retried. Check, October 6 at
    `24ba54689`: `cargo test -p loopflow --test task_flow_launch_tests` 7
    passed (three failures hold the edge stopped; failed, failed, succeeded
    arrives once with three Flow execs under one Task run). Not renamed
    yet: surfaces still say "Flow run"; slice 10's renaming owns it. Demo
    item: a real provider failure retried.
10. **Nodes and edges; Projects have workflows.** — **built, not yet
    compressed or realigned.** Jack, October 6: "it is ok for now to require
    that each edge is a unique step (flow/skill)"; "lets use Edge and node
    instead of Stage and Way OUt"; "Then projects have workflows instead of
    default" (both under "Name" and "Projects" in
    [task-workflow.md](task-workflow.md)). Built: node and edge in
    definitions (`nodes:`), types, store columns, wire, status text, skills,
    Desktop and docs, with no alias; "Flow exec" for one FlowExec record;
    the Project's `workflow` across the plan line, `lf wave update-plan
    --workflow`, status, wire, Desktop's Wave page and docs. A Task with no
    Workflow takes up its Project's; one that named its own keeps it.
    Done-when check, October 6: `cargo test -p loopflow --test
    task_flow_launch_tests` 8 passed (Project's workflow taken up; a named
    one kept; a Project naming a plain Flow refused); `--lib store::
    pm:: ops::chapter` and `--test dto_fixtures` pass; Swift
    `DesktopHeadlessTests|DTOFixtureTests|PodiumModelTests` 48 passed.
    Choices: [questions.md](questions.md). Demo items: the Wave page's
    Workflow section on screen; a live Project's `flow:` line read and
    rewritten.

10a. **A Task's state comes from its Workflow.** Jack, October 6: "no
    separate Task is ready state; that is derived from where it is in the
    graph"; "at start? ready at end? done"; between, "active";
    "abandoned/deleted bool should still stay"; then "I do think i want this
    in this PR now". His words on Linear and on Tasks with no Workflow are in
    the LOO-385 thread (`lf task comment LOO-385`); read them first.
    - State is read from position: ready at `start`, active between, done at
      `end`. Remove the stored ready/done status and every writer of it;
      abandoned and deleted stay flags.
    - Reaching `end`, by an edge or the set command, is completion.
      `lf task complete` goes; its settled-PR and unresolved-execution checks
      become refusals to reach `end`, and reaching it writes Linear.
    - Linear complete while the Task is ready: not offered as available.
      While active: a flag on the Task, shown as an error, work untouched,
      `end` refused until the flag is cleared. Clearing it, Jack: "Button on lf
      desktop / some lf task api", or alternatively "some sort of --force
      option to allow moving a run forward even when theres the flag". Build
      the second: `--force` on the command that reaches `end`, recorded in
      the move's note; Desktop's button passes it. No separate clear command
      (Jack, of `--force`: "i think i prefer that").
    - Agent's choices where Jack allowed either: a Task with no Workflow
      reads as not ready; creating a Task takes up its Project's workflow;
      existing Tasks get one by the draft migration only where their state
      is unambiguous (done → `end`, never started → `start`).
    Not in this slice: dropping the PR slot for a no-PR Workflow (LOO-385).
    Done when tests show state following position through start, a node, an
    edge and end; `end` refused over an unsettled PR; both Linear cases; and
    no surface or wire field carries a stored ready/done. *Not done.*

11. **Desktop draws the Workflow as a graph.** Jack, October 6, on the
    diagram in [pr-review.html](pr-review.html) ("The experience"): "this
    looks good, lets try to replicate this as a rough structural outline of
    what the loopflow desktop workflow viewer shows". Today Desktop lists
    stages with text rows ("→ pursue → demo"). Draw it: `start` and `end` as
    circles; each stage a box with its name and one line saying what the
    person does there; one labelled arrow per way out, a loop back drawn as
    an arc over its stage; the current stage or running edge marked; the ways
    out of the current stage are the buttons. Reuse the existing Flow graph
    renderer's drawing code. Agent's addition: a stage's one-line description
    is an optional field in the definition, else its skill's description.
    **Layout.** Jack: "we should put the graph side by side with a flow run
    log. that log should incldue anything run, regardless of what theworkflow
    was. it is *NOT* a workflow log". The graph sits beside a log of every
    Flow run in the Task, newest first, each opening to its steps from
    FlowExec. A run started ad hoc and a run that carried an edge are the
    same kind of row. The log is not the Workflow's move history and is not
    filtered or grouped by it. Open: whether a single headless skill Run
    with no Flow also belongs in that log.
    Done when the headless Desktop tests render `feature` and a no-PR
    definition from the shared fixture with the position marked. Demo item:
    how it looks. *Not done.*

12. **Align Desktop and the store with LOO-382.** Layer 4 of the contract in
    [task-workflow.md](task-workflow.md). If #1452 has merged, merge main and
    do all of it: domains and triggers for this PR's tables in the one draft,
    its store test listing them, Desktop's Workflow and Flow exec log fed by
    the `task` part, writes followed by `refresh`, and no per-view `lf task
    status`, `lf flow show` or catalog re-read on activation. If it has not
    merged, do what does not depend on it (stored state only; Desktop reads
    confined to one place in `RegistryQuery`) and record the rest as owed to
    whichever PR lands second. *Not done.*

Demo items carried from September 30, for Jack's review and not for the loop:
real provider continuation, owning-Home remote association, cross-Task
focus/input and process retention, same-byte symlink/read-only transitions,
twenty retained-layout actions against p95 <100 ms with idle CPU/process
counts. Details: [workspace review](../docs/reviews/task-workspace.md).

Exclusions: general messaging, automatic recovery, a Run entity, Project
reset (LOO-366), completion-policy redesign (LOO-367), Claude hooks/SDK,
external-progress credit from self-hosting, Task status presentation.

## Checks

Clear inherited `LF_*`/`LOOPFLOW_*` before Rust tests. Gate owns the full run.

| Command | Proves |
| --- | --- |
| `cargo test -p loopflow --test task_flow_launch_tests` | The entry, `--task` and a worktree launch run in the foreground, leave identical step commands and get identical refusals; a refused launch records no Flow. A Workflow's stored position: arrival, a stopped edge chosen again, `task move`, history. One Task run starting its Flow again: three failures hold the edge, a third-attempt success arrives once. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Against a fake provider answering in prose: decisions from the message contract, correction by `session resume` bounded at three turns, routing, three loop passes under one driver, a blocked decision. |
| `cargo test -p loopflow --test flow_tests --test flow_discovery_tests` | Flows read back from FlowExec; a killed driver's step stays recorded; a past Flow keeps its launched graph. |
| `cargo test -p loopflow --test land_tests lf_pr_land_returns` | A Flow stops at a landing its plain `pr land` step left watched. |
| `cargo test -p loopflow --lib store::` | Released frontier converts through the one draft; the record is append-only; a step must be its driver's child. |
| `cargo test -p loopflow --test dto_fixtures` | Wire shapes are unchanged. |
| `cargo test -p loopflow --lib harness::attention` | Recorded Claude, Codex and OpenCode streams under an injected clock: questions, open tools, hand-back, 120 seconds of quiet, a released driver, choice before paging. |
| `swift test --filter "DesktopHeadlessTests\|DTOFixtureTests"` (in `swift/`) | The workflow's position and edge Start, a Flow run's graph and steps, Waiting-first order and the Task primary, from the shared fixtures. |
| `git grep -nE "__flow-step\|FlowStep::\|execute_flow_command"` | No executable path. |

Configured demo, still separate: in a private Home, `lf -b task run INF-123
proof` prints and blocks; `lf monitor` and `lf flow show ID --sessions` show a
looping Flow's driver, step and iteration; a real provider step; Desktop.

October 6 sync onto main `5c0ca983a`: `cargo test -p loopflow --test session_lifecycle_tests` — 17 passed, 1 installation-only ignored; Session identity, caller provenance and historical-Exec acceptance reconciled with FlowExec.
