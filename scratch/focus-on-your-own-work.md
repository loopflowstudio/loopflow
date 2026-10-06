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

## Remaining from the FlowExec pass

Built at `aa2f42289` and `70e76a7d3`, unreviewed by Jack Heart; takeover and
resume are not built.

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

Slices 9, 9a and 10 are **done**; their text, checks and demo items are at
`65f8ec42b:scratch/focus-on-your-own-work.md`. 9. Workflow as live state
(`lf task move`, stored position, move history) — `5cf71ffd7`. 9a. One Task
run, many Flow execs (three attempts; run is the Workflow API's verb, exec the
Flow's record) — `274a63a8a`. 10. Nodes and edges; Projects have workflows —
`5b71ff345`, rerun October 6: `task_flow_launch_tests` 8 passed.

10a. **A Task's state comes from its Workflow** is **done**; its text is at
`9060bbea2:scratch/focus-on-your-own-work.md`. Jack, October 6: "no separate
Task is ready state; that is derived from where it is in the graph"; "I do
think i want this in this PR now". State is read from position,
`lf task move ISSUE end` replaces `lf task complete`, `tasks.work_state` is
dropped — `7f2c384c7`, rerun October 6: `task_flow_launch_tests` 13 passed,
`dto_fixtures` 20 passed. Demo items: Complete anyway on screen; a real
Linear completion against an active Task.

11. **Desktop draws the Workflow as a graph** beside a log of every Flow
    run is **done**; its text, Jack's two quoted requests and its choices are
    at `f9488a51d:scratch/focus-on-your-own-work.md`. Built `ade9146a5`,
    compressed `0d5da8058`; `swift test --filter
    "DesktopHeadlessTests|DTOFixtureTests|TaskFlowProofTests"` 36 passed.
    Jack's open question: whether a single headless skill Run with no Flow
    belongs in the Flow run log (default: no). Demo item: how it looks, in
    particular label widths, which are estimated from character counts.

Slices 12 and 13 are **done**; their text, checks and demo items are at
`7108dd3fb:scratch/focus-on-your-own-work.md`. 12. Align Desktop and the
store with LOO-382 (every new table has a revision domain; the Task page
reads only the stream's `task` part) — `c60fbf4f3`, compressed `a1b4ca012`.
13. Repair three failing tests found by running whole suites (an edge that
runs nothing ends a Task over uncommitted changes and keeps its checkout) —
`29b586dba`; nine Rust suites, 173 passed.

14. **Task view: the Workflow in the header, a multiplexer below.** Jack
    Heart, October 6, looking at the dev app at `4f2860658` (INF-123 at
    `demo`, three stopped `pursue` execs). His words, in order:
    - on the Task sheet: "this is kinda yucky";
    - on the workspace's Sessions sidebar: "dont need hte Sessions chrome
      after we have the main Task Session"; "thers just your Session and then
      you have your shells";
    - "and i want the graph in the header o this main view";
    - the Flow exec log: "should eventually go into this header, but that one
      i feel like should probably start hidden. put it behind a button in the
      header chrome"; then "maybe the runlog is more of a multiplexer entry
      than part of the header";
    - the worktree file browser: "i would like to add that as well";
      "another thing we need a button for in the chrome is the file viewer";
      "yes, its also a multiplexer plane";
    - summing up: "the workflow grpah is aprt of the header, and tehn a bunch
      of chrome for changing whats in the multipleer";
    - on the toolbar's Monitor button and its pane ("No active Sessions in
      this observation"): "this monitor button seems like its an older
      attempt at the runlog?" It is: the Flow exec log pane replaces Monitor. Jack: "Delete this
      then": the Monitor pane, its button and its reader are deleted;
    - on the sidebar toggle: "session hider goes away. merge the + button
      with the file and then whatever for run log": one **+** menu adds a
      shell, the file viewer or the Flow exec log; no separate file, Monitor
      or sidebar buttons;
    - on the toolbar's Flow chip ("task wait NOPE-1 --timeout 1"): "this is
      replaced with the much bigger flow graph";
    - on the worktree chip reading `.tmp5q776G`: "this .tmp is weird ?" It is
      the checkout's directory name, odd only because the fixture lives in a
      temporary directory; no change.
    **Done** at `975d9f727`, marked by the managing conversation after
    rerunning nine suites and looking at the render. What was built, removed
    and proved is at `f8fe8aff0:scratch/focus-on-your-own-work.md` under
    this slice; choices are in [questions.md](questions.md). Demo items: the
    window on screen; a real `lf session ensure` starting a provider; label
    widths at small window sizes.

15. **Merge main** is **done**: `b5948ab72` (#1463, by the pass) and
    `f360cdcc3` (#1468, #1469, #1449, by the managing conversation). Jack:
    "Determine the higher level goal of the diff and make suer we have a
    nother way of addressing ... ideally is just a bunch unncessary code that
    is made trivial by the new model." Finding: #1463 (LOO-366: a Wave's
    Project selection in SQLite, `lf wave bind-project`/`ensure`, Desktop
    preparing a Project on opening, rotation and Realign Projects, the
    workspace→work rename) needs none of the deleted machinery; only tests
    used the worker to stage work in flight. Rerun October 6 at `f360cdcc3`:
    all 56 integration suites, `--lib ops::chapter` alone (25), Clippy, 75
    headless Desktop tests pass. In the full parallel library run 11
    `ops::chapter` tests fail (10 rotation, 1 adoption) that pass alone.
    Unreviewed: main lets a Project have no workflow; its Tasks read not
    ready.

16. **Cleanups after Jack's approval** ("make whatever cleanups yyou want")
    is **done**: built `638fe0e6c`, compressed `25ce573aa`; its text is at
    `25ce573aa:scratch/focus-on-your-own-work.md`. Queued at `f8fe8aff0`,
    absent at `f9e74029e` with no reason recorded, restored by the agent.
    Text `lf task status` heads a Task with its Workflow state; loop
    positions read "pass 2" in Desktop, `lf flow show` and `lf monitor`; the
    fixture's Flow exec is a `pursue` run on its second pass. Swift was not
    searched for unused code. Rerun October 6 at `6e390e513` (main's #1470
    and #1471, release scripts only, merged without conflict):
    `cargo test -p loopflow --test dto_fixtures --test flow_tests --test
    task_flow_launch_tests` 20, 27 and 13 passed. Demo item: the pass
    wording on screen.

Every slice in this list is done.

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
| `swift test --filter "DesktopHeadlessTests\|DTOFixtureTests\|TaskFlowTests"` (in `swift/`) | The workflow's position and edge Start, Waiting-first order and the Task primary, from the shared fixtures; a Task's work, a move and a new Flow run arriving from stream frames; the Task workspace opening on its ensured Session with the Workflow header and the exec log and files as panes. |
| `cargo test -p loopflow --lib store::sqlite::revisions` | Every table has a revision domain; the tables this PR adds name theirs; a Session reading moves `sessions` only when Waiting could change. |
| `git grep -nE "__flow-step\|FlowStep::\|execute_flow_command"` | No executable path. |

Configured demo, still separate: in a private Home, `lf -b task run INF-123
proof` prints and blocks; `lf monitor` and `lf flow show ID --sessions` show a
looping Flow's driver, step and iteration; a real provider step; Desktop.
