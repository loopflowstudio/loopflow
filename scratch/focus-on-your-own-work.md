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

FlowExec and oblivious steps are built (`aa2f42289`, `70e76a7d3`), and
TaskWorkflow after them; Jack Heart has reviewed neither. Takeover and resume
are not built.

- **Task entry.** `lf task run ISSUE [FLOW]` places the Task (worktree,
  `--stack-on`, checkout restore), defaults to the Project's Flow, records `-m`
  as the Task's agent and `--reason` as a steer, then continues as
  `lf --task ISSUE run FLOW`. It honors `-b`/`-i`, prints and blocks.
- **One path.** Every Flow launch that resolves to a Task, by the entry,
  `--task` or its worktree, gets the same check: ready Work, matching
  planning, current chapter, not being abandoned.
- **FlowExec.** `flow_execs` (driver Exec, Flow name, graph compiled at launch)
  and `flow_exec_steps` (child Exec, graph node key, per-edge iterations), both
  append-only by trigger; a step row must name an Exec its driver started. The
  driver writes the Flow row before its first step, marks its Task started,
  and appends each step once the child's Exec appears.
- **Oblivious steps.** A skill step is `lf -b [options] skill <name>
  [message]`; an operation is its own `lf` command through the ordinary CLI;
  a correction is `lf -b session resume SESSION MESSAGE`. Gone: `FlowStep`,
  `--__flow-step`, the `__flow-step` command, `--__cwd` on steps, exit status
  75, the in-process operation interpreter and its allowlist, per-node steer
  acknowledgement, the Session's captured Flow position, and provider
  structured-output requests (`--json-schema`, `outputSchema`, OpenCode
  `format`). The answer contract is in the step's message; the driver reads
  the turn's final answer and accepts JSON inside prose or a code fence.
- **Readers.** `lf flow list/show`, `task status`, completion blockers,
  chapter "started", `lf monitor`, Session Flow membership and the Desktop
  DTOs read FlowExec joined to Execs. Wire shapes are unchanged, so fixtures
  and Swift decoders are untouched. A past Flow keeps its launched graph.
- **Migration.** The one draft also creates the two tables and reads Started
  evidence from a Flow run in the Task's checkout.

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
- **Leftovers.** Native history readers still parse `structured_output`, for
  conversations recorded before this pass;
  `SessionFlowMembership::Step` remains for older manifests.
  `SessionKind::FlowReview` and `SessionAttention::Review` go with the Waiting
  slice. Task status still lists every Exec. `--needs-me` remains until
  `--waiting` replaces it. Desktop does not draw step Execs.
- **Failing before this pass, unchanged** (reproduced on `8535598ef`):
  `session_cli_tests resume_selects_human_input_in_the_physical_worktree_and_records_opening`
  and `status_tests previous_release_merge_request_migrates_into_readable_status_and_roadmap`.

## Slices — one looping Flow builds these in order

Jack Heart, October 5: "i want you to manage a looping lf flow that implemetns
(and compresses, and realigns, etc.) all the slices". One `pursue` run loops:
each pass builds the first slice not marked done, compresses, syncs and
realigns; realign marks the slice **done** here with its commit and one-line
check result; the deciding step iterates while any slice is not done and
advances to publish only when all eight are; the list is the authority on how many there are. A proof that needs a person, a display
or a live provider is listed under the slice as a demo item and never keeps a
slice open. A pass that cannot finish its slice records why and stops blocked.

Flow history below means FlowExec rows joined to their Execs.

1. **TaskWorkflow.** Build order, defaults and tests are in
   [task-workflow.md](task-workflow.md). Done when its five planned tests pass and
   `task status --json` carries the workflow graph, position and running
   edge. **Done** at `1f665b901` (the Flow stopped at sync before realign; marked by the managing conversation after rerunning the launch and workflow tests):
   `task_flow_launch_tests` 5 passed, lib `engine::workflow`/`ops::task_workflow`
   3 passed (stage guidance included), `dto_fixtures` 18 and Swift
   `DTOFixtureTests` 22 passed, all-target Clippy clean. Demo items: a real
   provider run of `feature`'s `task-design` edge; the stage guidance as a
   live Task conversation reads it.
2. **Loops.** One form: a deciding node last, `loop: <target>` with optional
   `step` defaulting to `loop-or-next`, replacing `loop-decide` and
   `repeat.from` in catalog, skills, templates, docs and tests with no alias.
   Unique skill names resolve the target; one optional occurrence name
   disambiguates. Advance, Iterate or Blocked; malformed output never
   advances; shared and overlapping return edges are preserved. Done when
   `git grep -E "loop-decide|repeat:"` matches only released migrations and
   dated reviews, and `flow_tests` covers three nested loops. *Not done.*
3. **Review repairs.** Two defects found on October 6. (a) Steers: Jack
   Heart, on every step now receiving all of its Task's steers: "This seems
   potentially bad as that was specifically added to address some runaway
   token counts". Restore the saving without telling a step about its Flow:
   the driver, which knows when it last ran this node, passes an ordinary
   public option on the step's command that limits Task direction to steers
   newer than a given one; the option works on any `lf` run. (b) A flowless
   edge into `end` cannot be taken from a stage with another way out:
   `lf task run ISSUE` answers "more than one outgoing edge; name its Flow"
   and `end` is not a Flow, so builtin `research` can never end. Let
   `lf task run ISSUE end` (the target stage's name) take it, and say so in
   the refusal and the stage guidance. Done when a test shows a node's second
   run receives only steers newer than its first, and a test walks `research`
   to `end`. *Not done.*
4. **Task primary.** `PrimaryScope::Task(TaskId)` under the existing scope
   lock. Explicit choice wins; else the sole unfinished interactive Task
   conversation; else the most recent by `latest_interactive_session`'s
   ranking, restricted to unfinished Task members. Read-only inventory creates
   nothing. Done when `session_lifecycle_tests` covers each rule. *Not done.*
5. **Waiting.** Rust-owned; replaces Review/Reply attention,
   `SessionKind::FlowReview` and `--needs-me` with one Waiting value and
   `--waiting`. Immediate on explicit pending input or a successful
   interactive yield with no outstanding tools; otherwise after 120 seconds
   without provider activity and zero unresolved tool calls. No event yet is
   opening/unknown; disconnection is unknown; new activity clears it;
   filtering happens before paging. Provider evidence:
   [harness-attention.md](harness-attention.md). Done when the injected-clock
   and recorded-trace tests pass for Claude, Codex and OpenCode. *Not done.*
6. **Desktop.** The Task page draws its TaskWorkflow with the current stage
   or running edge, and every Flow run of the Task gets the same
   graph/progress/output view from FlowExec. Waiting first, working Sessions
   in a compact group, no completion controls; Start runs `lf -b task run` as
   an app-owned child. Done when `swift build --build-tests` and the headless
   Desktop tests pass with shared DTO fixtures. Demo items: native rendering
   and interaction. *Not done.*
7. **Defaults and editing.** The Project's default names a workflow, shown and
   set in Wave settings; Edit opens the real workflow or Flow source, with
   builtin customization creating the `.lf/` file explicitly; an invalid file
   stays saved and visibly invalid. Done when its headless tests pass.
   *Not done.*
8. **Docs and website.** README, `docs/`, AGENTS.md, the architecture
   reference, builtin skills and the website describe TaskWorkflow, FlowExec,
   Sessions and Runs, Waiting and `lf task run`; no resident, review-handshake
   or worker text remains. Done when the website checks and
   `scripts/check_architecture.py` pass. *Not done.*

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
| `cargo test -p loopflow --test task_flow_launch_tests` | The entry, `--task` and a worktree launch run in the foreground, leave identical step commands and get identical refusals; a refused launch records no Flow. |
| `cargo test -p loopflow --test session_lifecycle_tests` | Against a fake provider answering in prose: decisions from the message contract, correction by `session resume` bounded at three turns, routing, three loop passes under one driver, a blocked decision. |
| `cargo test -p loopflow --test flow_tests --test flow_discovery_tests` | Flows read back from FlowExec; a killed driver's step stays recorded; a past Flow keeps its launched graph. |
| `cargo test -p loopflow --test land_tests lf_pr_land_returns` | A Flow stops at a landing its plain `pr land` step left watched. |
| `cargo test -p loopflow --lib store::` | Released frontier converts through the one draft; the record is append-only; a step must be its driver's child. |
| `cargo test -p loopflow --test dto_fixtures` | Wire shapes are unchanged. |
| `git grep -nE "__flow-step\|FlowStep::\|execute_flow_command"` | No executable path. |

October 5, after compress: the commands above except `land_tests` passed
(`store::` 181, `session_lifecycle_tests` 17, `flow_tests` 24,
`flow_discovery_tests` 3, `task_flow_launch_tests` 1, `dto_fixtures` 18);
all-target Clippy clean. `land_tests` and the full suites are gate's.

October 6, Loops (slice 2), before compress: `--lib engine::` 414,
`flow_tests` 25 six times running (three nested loops included),
`session_lifecycle_tests` 17, `task_flow_launch_tests` 5,
`flow_discovery_tests` 3, `dto_fixtures` 18, `golden_prompt` 1 passed;
all-target Clippy clean. Swift fixtures changed a label only and were not
built. The agent's choices are in [questions.md](questions.md).
After compress (one `return_target` shared by graph, transition and driver):
`--lib engine::` 414, `flow_tests` 25, `session_lifecycle_tests` 17 passed;
all-target Clippy clean.

Configured demo, still separate: in a private Home, `lf -b task run INF-123
proof` prints and blocks; `lf monitor` and `lf flow show ID --sessions` show a
looping Flow's driver, step and iteration; a real provider step; Desktop.
