# Open choices — October 5, 2026

For [the design](focus-on-your-own-work.md). The pass followed each default
below; defaults and interpretations are the agent's, not Jack Heart's. Earlier
interpretations are at `6f246fda4:scratch/questions.md`.

## Answered by Jack Heart (October 5)

- **Who backgrounds, now that `-b` blocks?** "cant you just background with
  &" and "like llms know how to background things". The caller does; there is
  no detached launch mode. Explicit `nohup`/`&` examples in skills are fine.
  The six builtin skills that say "background it with your own tool" stay.
- **Task helpers.** On deleting `task create --run`: "Im fine with this. i am
  fine with task helpers that sit on top of the lf system, but i dont want to
  introduce parallel paths or drivers." A Task command may prepare, then it
  enters the ordinary path; it never owns a second launcher, driver or record.

- **Steers per step (October 6).** On the FlowExec pass dropping per-node
  steer acknowledgement: "This seems potentially bad as that was specifically
  added to address some runaway token counts". Repair is slice 3 in the
  design; the mechanism there (a public newer-than option the driver passes)
  is the agent's.

## Step invocation — decided by Jack Heart (October 5)

At `45ca0d2ac` the driver passes each step a JSON `FlowStep` through the
hidden `--__flow-step` argument (the hidden `__flow-step` command for
operations); readers find Flows by searching recorded command text.

Jack: "Can we do this environemnt variables? I hate __ and hidden arguments."
The agent proposed an environment variable plus a nullable Exec field. Jack
rejected both halves:

1. "the skill is its own argument. we should use lf from flows in the
   'natural' way, invoking the right skill".
2. On needing a field for position: "right, but execs are supposed to trakc
   their parents already".

So a step is the command a person would type, started as a child of the
driver: `lf -b skill implement [message]`, `lf task sync --plan`. No
`FlowStep` payload, hidden argument, hidden command, position variable or new
Exec field. A Flow is read from the driver Exec, its children in start order
and their commands; iteration is the count of earlier siblings with the same
command.

3. "why does the skill lf exec even need to know what step position it is?
   just runt the right skill. it doesnt need to know its part of a flow to
   run".
4. "its fine if a flow process does extra logging in lfdb or the file system,
   or maintains state, to make querying about active/completed flows easier.
   but i think that sould happen in the flow, not the step".

5. "we should make the step part of a flow as radically simple as a normal
   flow, and put the flow logic in the flow process".

6. "to the extent that there was useful stuff going on with the flowsession
   before, it should be part of any lf flow. for now we drop any notion of
   'one primary flow' for a task, and we might add that back on as a layer on
   top once we just have generally 'you can run flows for any tasks, and we
   will do good tracking for any flow you run adhoc just the same as the
   standard operating protocol one that is run aautomatically when you start
   a task'".

What this settles: tracking is uniform. Every Flow run, ad hoc or the
Project's default started by `lf task run`, is tracked the same way; no Flow
is primary for a Task. A primary Flow may return later as a layer on top.

7. Confirming the reading below: "Yeah, we should be able to do taht stuff
   for any running flow. the flow driver should be responsible for maintaining
   the state to make that queryable and scalable and performant; the 'client'
   code of it should be made to work on top of any flow". And: "we *dont*
   right now need to make any flow like mutable on demand by some sort of API
   - read only other than the parent process ctrl-cing it or whatever".

Decided, then: the driver maintains queryable Flow state for every Flow;
readers (`flow list/show`, `task status`, Desktop) are clients of that state
and work for any Flow; the state is read-only to everyone but the driver. The
only control is ending the driver process.

8. On a record that extends the driver Exec: "its not exactly 1:1 with exec
   in the same way that a simple interactive skill session is not 1:1 with
   its launching process". Then: "ok so yeah maybe we still want FlowSession.
   And then the FlowSession is mutable, but the Flow exec is not".
9. "but just in the same way the session api lets you replace / take over any
   lf skill, the flow session api would let you replace / take over any
   running flow".

10. "i have also previously talked about the idea of there being two different
    notions of flow. One where there there is a start and a land node and
    then in between are human sessions, and the edges are lf flows". Then: "I
    think maybe the flowsession is *that*".

11. "i think maybe we call that like a TaskWorkflow or something ? might need
    to think about ones that dont end in land, multi-PR tasks or 0-PR tasks.
    (I am pretty confident we want 0 PR tasks; not sure we need multi-PR
    tasks)".

12. On the outer/inner two-record table: "your table looks great. lets start
    to center on tat".

13. On keeping this PR to the inner record and leaving the outer one as the
    next Task: "Yeah i think so." On the name: "FlowExec works fine".

Where the outer model already appears (searched October 5): the September 30
design (`bc78c27c0:scratch/growth-thoughts.md`: "interactive skills are
nodes; background Flows are the edges"), the October 4 kickoff
(`5090f672e:scratch/focus-on-your-own-work.md`), and open Tasks LOO-317 (Mac
shows a flow as only the steps that need you), LOO-297 (design or prototype
review paths), LOO-322 (reach demo and landing unattended) and LOO-367. No
Task owns TaskWorkflow. Conflict to resolve there: the September 30 design
and LOO-367 both record Jack saying there is no separate "Task workflow"
concept.

Resolution, centered on that table (the names TaskWorkflow, from Jack, and
FlowExec, the agent's, postdate it): the outer thing is TaskWorkflow and
is not built in this PR; one `lf` flow run gets an append-only driver record
named FlowExec.

The consolidated model is in [the design](focus-on-your-own-work.md)
under "Flow model".
Takeover and resume are not built: Jack's request for the FlowExec pass
excluded them along with TaskWorkflow.

The rule: Flow bookkeeping belongs to the driver process. It may record what
it started and where it is (journal events, rows or files it writes itself).
A step is told nothing and writes nothing about the Flow.

The agent's readings of these points, since built, are at
`b30b193a0:scratch/questions.md`.

## Asked of Jack, unanswered; the pass took the default

- **Planning refusal** (terminal, moved or removed Task) applies to every Flow
  launch that resolves to a Task. It does not apply to a single skill or an
  interactive conversation in a Task worktree: binding stays allowed for done
  Tasks.
- **`task automate`/`automation`, `task interrupt`, `task wait`** are kept as
  ordinary Task commands.
- **The Task entry is `lf task run ISSUE [FLOW]`.**
- **A past Flow after its YAML changed** keeps the graph FlowExec captured at
  launch (the earlier pass redrew it from the Exec sequence).
- **`lf commit`:** `-p` pushes; plain commit stays local.

## Choices the FlowExec pass made without Jack

The agent's, October 5; none is confirmed by Jack Heart. The full list is at
`4207263c5:scratch/questions.md` under this heading. The ones most likely to
matter in review: the driver finds each step's Exec by polling, so a driver
killed in that gap leaves a step unrecorded; operations run through the
ordinary CLI, so any `lf` command can be a `cmd:` step and `cmd: pr open`
opens the browser; the answer contract is message text only, with provider
structured-output requests deleted; a step looks its skill up by name, so an
edit mid-Flow applies to later steps; `LF_FLOW_ID` stays in a step's
environment for the S1–S5 shared note; a past Flow keeps its launched graph.

## Choices the TaskWorkflow pass made without Jack

The agent's, October 5, building slice 1. None is confirmed by Jack Heart.

- **`feature` and `code` are workflows only.** The builtin Flows of those
  names are deleted. `lf run feature` fails and names `lf task run <issue>
  feature`; it is not rerouted through the Task entry, inside a Task or out.
- **A repository Flow keeps its name.** `.lf/flows/feature.yaml` hides the
  builtin workflow `feature`, so a repository that customized it runs it ad
  hoc as before. `.lf/workflows/feature.yaml` wins over both.
- **Which Tasks get a workflow.** One whose Project default names a workflow,
  on its first `lf task run`; or one that names a workflow explicitly. A
  Project whose default is a plain Flow leaves its Tasks ad hoc.
- **Naming a workflow replaces.** `lf task run ISSUE WORKFLOW` on a Task with
  another workflow starts the named one at `start`; the earlier rows stay. A
  name that is both an outgoing edge's Flow and a workflow means the edge.
- **The traversal names the `lf task run` Exec,** which is the edge Flow's
  driver, so the row needs no FlowExec to exist yet. A launch refused after
  the row is written leaves a failed traversal: position unchanged.
- **Position.** At `to` only when that Exec succeeded. A driver with no exit
  record counts as running unless its process is known dead. A Flow that
  stops at a watched landing has not succeeded, so `ship` returns the Task to
  `demo` until a later run of it succeeds.
- **No guard on a running edge.** A second `lf task run` while one runs
  chooses among the edges leaving the running edge's `from`.
- **Edges accept what `lf run` accepts,** a skill included: `research` runs
  the `research` skill. Flowless edges are allowed only into `end`.
- **`research` names a workflow and a skill.** `lf research` stays the skill.
- **Wire placement.** The workflow rides in `TaskWork`, not the Task Flow
  snapshot, so the roadmap does not carry it; Desktop reads it per Task.
- **Guidance** is appended wherever a Task binding's context is rendered
  (`resolve_work_binding`), headless launches included.
- **Skill prose.** Only `advance` and `launch-plan` were corrected for the
  deleted Flows; other skills still say `lf task run <issue> <chosen-flow>`,
  which a Task on a workflow refuses unless that Flow leaves its stage.
- **Flow catalog.** `lf flow list` no longer lists `feature` or `code`, so a
  Project default of `feature` has no Flow graph; Desktop draws the workflow
  once the Task's first run records it.
- **Test count.** The slice list said six tests; the build order at
  `26279a3d2:scratch/task-workflow.md` lists five behaviors, each covered.
  The list now says five.
- **Reaching `end` removes nothing:** the Task stays ready and
  `lf task complete` stays separate (draft default 3).

## Choices the Loops, Review repairs and Task primary passes made without Jack

The agent's, October 6, slices 2–4; none is confirmed by Jack Heart. The full
lists are at `f38e3e2c9:scratch/questions.md` under each pass's heading. The
ones most likely to matter in review:

- **Loops.** `- loop: <target>` with an optional sibling `step:`; the
  occurrence name is the existing `id`; `repeat:` is refused with a message
  naming `loop:`; a `human: true` decider is refused.
- **Steers.** Global `--steers-after STEER`; the driver passes each node its
  previous run's newest steer, so a steer can arrive twice, never zero times.
- **Ending.** `lf task run ISSUE end` takes an edge that runs nothing.
- **Task primary.** `tasks.primary_session_id`; `lf session ensure --task
  ISSUE [--choose SESSION]` remembers its first pick; only an empty Task gets
  a new conversation; `lf resume` ignores the primary.
- **Seen, not fixed.** `scripts/check_architecture.py` reports the two
  workflow tables missing from the map (docs slice). Five lib tests fail in a
  full parallel `--lib` run and pass alone.

## Choices the Desktop and Waiting passes made without Jack

The agent's, October 6, slices 5–6; none is confirmed by Jack Heart. The full
lists are at `574858a7c:scratch/questions.md` under each pass's heading. The
ones most likely to matter in review:

- **Waiting comes only from a stream `lf` owns:** every headless Run, and
  Codex through the app-server. A native `claude` or `opencode` terminal never
  shows Waiting. Open for Jack: reading Claude's transcript file, which his
  stream-json-only rule may exclude.
- **One mutable reading per Session** (`session_activity`), saved at most
  every 5 seconds; one SQL rule judges the row and `--waiting`: an unanswered
  question, or no open tool call and either an interactive hand-back or 120
  seconds of quiet. A killed driver's reading stays in force.
- **Codex approvals count as questions;** OpenCode permissions do not.
- **Wire.** `attention` is `"waiting"` or null; Session `kind`, state
  `waiting`, `TaskSession.kind` and the `agent_sessions.kind` column are
  gone; `task_primary` marks a Task's primary on every listed Session.
- **Desktop matches no conversation to a Flow step** and never calls
  `session ensure`. Each Flow run is a row that opens to `lf flow show ID
  --sessions --json`; `current` draws as running, so a killed driver's run
  reads as running forever.
- **Workflow Start** is one button per edge leaving the waiting stage, all
  disabled while an edge runs; `lf` itself still allows a second run.
- **Seen, not fixed.** `ActiveSessionsLifetimeTests replacement` fails in a
  full parallel `swift test` and passes alone.

## Choices the Defaults and editing pass made without Jack

The agent's, October 6, building slice 7. None is confirmed by Jack Heart.

- **"Wave settings" is the Wave page's Task default section;** Desktop has no
  settings screen. Its menu sets the default, and lists Flows after workflows
  because a Project default may still be a plain Flow.
- **One catalog.** `lf flow list --json` lists workflows beside Flows: each
  entry gains `kind`, `source` (the repository file, null for a builtin) and
  `workflow` (stages and edges). Text output marks `(workflow)`. An unstarted
  Task previews its default workflow from it.
- **`lf flow customize NAME`** prints the repository file, writing the
  builtin to `.lf/workflows/` or `.lf/flows/` when there is none; a workflow
  wins over a Flow of the same name. Desktop's Customize (builtin) and Edit
  (repository file) both call it, so only the label differs.
- **Edit opens the file in the system's default application,** not the Task
  file pane, and in the checkout Desktop has open for the repository.
- **`lf wave update-plan --flow NAME`** rewrites only the `flow:` line. It
  does not check that NAME loads: the CLI can save a default that names
  nothing, which Desktop shows as invalid with Start disabled.
- **Invalid files** stay listed with their reason and stay editable; Desktop
  rereads the catalog when it becomes the active application, not on a timer.
- **Not listed:** a skill used as a default, which `lf run` accepts.

## Choices the Docs and website pass made without Jack

The agent's, October 6, building slice 8. None is confirmed by Jack Heart.

- **Operators on a workflow.** Jack's October 5 "a defined Flow with steps
  left proceeds without asking" had no mechanism after resume was deleted.
  The operate skills and docs now say: a Task at a stage its last edge
  reached waits on a person; an edge that stopped or failed is rerun after
  its log is read; no other edge is taken for the person.
- **Skills name no Flow by default.** `lf task run <issue> <chosen-flow>`
  became bare `lf task run <issue>`, with one paragraph on edges, `end`,
  taking up a workflow and `lf --task <issue> run <flow>` for a Flow that
  must not move the Task.
- **"Worker" survives** in skills where it means whoever does the work.
- **Website.** The flow example is builtin `pursue`; a fifth block shows the
  `feature` workflow and an odd last block spans the grid. The SVG diagrams
  are not redrawn.
- **Architecture map.** TaskWorkflow is its own concept row.
- **Not written:** a standalone Waiting or Desktop guide; `docs/lf.md`,
  `conducting.md` and `swift/README.md` carry them.

## Choices the earlier pass made without Jack

- **A killed driver leaves a running agent turn to its Session.** Nothing reads
  that turn back into the Flow.
- **Agent preflight before placement is gone.** An unavailable provider or
  account now fails at the first step, in the foreground.
- **Launch-time PR preparation is gone.** Publication refuses a settled PR and
  names `lf pr next`.
- **Flow states** are `current`, `completed`, `stopped`; `replaced` is gone.

## Interpretations still awaiting Jack's review

- Builtin `feature` ends at a published PR with no design-review pause; `ship`
  lands after the demo; `code` equals `pursue`. Open: whether `feature` should
  stop after kickoff, and whether `code` should remain.
- Main's `lf flow end` (#1435, LOO-326) is dropped: a stopped Flow blocks
  nothing. Open: whether Infrastructure wants a mark on stopped history.
- The per-Task automation on/off field is kept for CI repair only.
- Implementation defaults for unbuilt slices: 120-second Waiting fallback,
  most-recent interactive Task primary, optional occurrence names,
  backward-target loops, source-only workflow YAML.
- Global `-w` and `lf session open`, also removed by #1356, are left alone as
  outside this Task.
